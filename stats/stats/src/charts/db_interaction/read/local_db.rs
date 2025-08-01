/// Methods intended for interacting with local db
use crate::{
    ChartError, ChartProperties, MissingDatePolicy,
    charts::{ChartKey, chart::ChartMetadata},
    data_source::kinds::local_db::parameter_traits::QueryBehaviour,
    missing_date::{fill_and_filter_chart, fit_into_range},
    range::exclusive_range_to_inclusive,
    types::{
        ExtendedTimespanValue, Timespan, TimespanDuration, TimespanValue,
        timespans::{DateValue, Month, Week, Year},
    },
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::{
    chart_data, charts,
    sea_orm_active_enums::{ChartResolution, ChartType},
};
use itertools::Itertools;
use sea_orm::{
    ColumnTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, FromQueryResult, QueryFilter,
    QueryOrder, QuerySelect, Statement, sea_query::Expr,
};
use std::fmt::Debug;
use tracing::instrument;

use super::ReadError;

#[derive(Debug, FromQueryResult)]
struct ChartID {
    id: i32,
}

pub async fn find_chart(db: &DatabaseConnection, chart: &ChartKey) -> Result<Option<i32>, DbErr> {
    charts::Entity::find()
        .column(charts::Column::Id)
        .filter(
            charts::Column::Name
                .eq(chart.name())
                .and(charts::Column::Resolution.eq(ChartResolution::from(*chart.resolution()))),
        )
        .into_model::<ChartID>()
        .one(db)
        .await
        .map(|id| id.map(|id| id.id))
}

/// Get counter value for the requested date
pub async fn get_counter_data(
    db: &DatabaseConnection,
    name: &str,
    date: Option<NaiveDate>,
    policy: MissingDatePolicy,
) -> Result<Option<DateValue<String>>, ReadError> {
    let current_date = date.unwrap_or(Utc::now().date_naive());
    let raw_data = DateValue::<String>::find_by_statement(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
            SELECT distinct on (charts.id) data.date, data.value
            FROM "chart_data" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE
                charts.chart_type = 'COUNTER' AND
                charts.name = $1 AND
                data.date <= $2
            ORDER BY charts.id, data.id DESC;
        "#,
        vec![name.into(), current_date.into()],
    ))
    .one(db)
    .await?;
    let data = raw_data.map(|mut p| {
        if policy == MissingDatePolicy::FillZero {
            p.relevant_or_zero(current_date)
        } else {
            p.timespan = current_date;
            p
        }
    });
    Ok(data)
}

/// Mark corresponding data points as approximate.
///
/// Approximate are:
/// - points after `last_updated_at`
/// - `approximate_until_updated` points starting from `last_updated_at` and moving back
///
/// If `approximate_until_updated=0` - only future points are marked
fn mark_approximate<Resolution>(
    data: Vec<TimespanValue<Resolution, String>>,
    last_updated_at: Resolution,
    approximate_until_updated: u64,
) -> Vec<ExtendedTimespanValue<Resolution, String>>
where
    Resolution: Timespan + Ord,
{
    // saturating sub/add
    let next_after_updated_at = last_updated_at.saturating_next_timespan();
    let mark_from_timespan = next_after_updated_at.saturating_sub(
        TimespanDuration::from_timespan_repeats(approximate_until_updated),
    );
    data.into_iter()
        .map(|dv| {
            let is_marked: bool = dv.timespan >= mark_from_timespan;
            ExtendedTimespanValue::from_date_value(dv, is_marked)
        })
        .collect()
}

/// Note: timestamp has microsecond precision
/// (postgres default timestamp)
pub async fn get_chart_metadata(
    db: &DatabaseConnection,
    chart: &ChartKey,
) -> Result<ChartMetadata, ReadError> {
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(
            charts::Column::Name
                .eq(chart.name())
                .and(charts::Column::Resolution.eq(ChartResolution::from(*chart.resolution()))),
        )
        .one(db)
        .await?
        .ok_or_else(|| ReadError::ChartNotFound(chart.clone()))?;
    Ok(ChartMetadata {
        last_updated_at: chart.last_updated_at.map(|t| t.with_timezone(&Utc)),
        id: chart.id,
        created_at: chart.created_at.with_timezone(&Utc),
    })
}

/// Returns tuple with:
/// - latest resolution that has relevant data
/// - does # of approximate points needs to be decreased by 1
fn relevant_data_until<R: Timespan>(
    last_updated_at: Option<DateTime<chrono::FixedOffset>>,
) -> (Option<R>, bool) {
    if let Some(t) = last_updated_at {
        let t = t.to_utc();
        let last_updated_at_timespan_border =
            R::from_date(t.date_naive()).saturating_start_timestamp() == t;
        // last_updated_at timestamp is not included in the range
        let inclusive_last_updated_at_end =
            exclusive_range_to_inclusive(DateTime::<Utc>::MIN_UTC..t);
        (
            Some(R::from_date(
                inclusive_last_updated_at_end.end().date_naive(),
            )),
            last_updated_at_timespan_border,
        )
    } else {
        (None, false)
    }
}

/// Get data points for the chart `name`.
///
/// `approximate_trailing_points` - number of trailing points to mark as approximate.
///
/// `point_limit` - max interval [from, to]. If `from` or `to` are none,
/// min or max date in DB are taken.
///
/// Note: if some dates within interval `[from, to]` fall on the future, data points
/// for these dates are not returned (makes sense, since it's future).
///
/// ## Missing points and `fill_missing_dates`
/// If `fill_missing_dates` is `false`, some dates might be missing from the result.
/// It means these dates have values according to corresponding underlying `MissingDatePolicy`
/// (in particular, the one that was set during update (should be == to `policy` in practice)).
///
/// Because of this, returned data may slightly differ from records in DB. In any way, the data
/// is semantically equivalent.
///
/// `fill_missing_dates=true` takes care of it. In this case, values for the whole range
/// are returned (except for the future).
///
/// ### `MissingDatePolicy::FillPrevious` example
///
/// DB data (empty value means no record in DB):
///
/// ```text
///   date  | 1 | 2 | 3 | 4
///  -------|---|---|---|---
///   value | A |   | B |   
/// ```
/// (2 records in DB: `1: A`, `3: B`)
///
/// `get_line_chart_data` with range `2..=4` and `fill_missing_dates=false` will return 2 records :
/// `2: A`, `3: B`. It can be represented like this:
/// ```text
///   date  | 2 | 3 | 4
///  -------|---|---|---
///   value | A | B |   
/// ```
///
/// - Value `A` is moved to correctly represent value at date `2`. Value for `1`
///   is outside the range, thus we move the value.
/// - Date 4 is still omitted, because it can be calculated from `3`
///
/// with `fill_missing_dates=true`:
/// ```text
///   date  | 2 | 3 | 4
///  -------|---|---|---
///   value | A | B | B
/// ```
///
/// `get_line_chart_data` with range `0..=2` and `fill_missing_dates=true`
/// ```text
///   date  | 0 | 1 | 2
///  -------|---|---|---
///   value | 0 | A | A
/// ```
///
/// ### `MissingDatePolicy::FillZero` example
///
/// DB data (empty value means no record in DB):
///
/// ```text
///   date  | 1 | 2 | 3 | 4
///  -------|---|---|---|---
///   value | A |   | B |   
/// ```
/// (2 records in DB: `1: A`, `3: B`)
///
/// `get_line_chart_data` with range `2..=4` and `fill_missing_dates=false` will return 1 record :
/// `3: B`. It can be represented like this:
/// ```text
///   date  | 2 | 3 | 4
///  -------|---|---|---
///   value |   | B |   
/// ```
///
/// - Caller can deduce that no record for date `2` means that value there is 0
///
/// with `fill_missing_dates=true`:
/// ```text
///   date  | 2 | 3 | 4
///  -------|---|---|---
///   value | 0 | B | 0
/// ```
///
#[allow(clippy::too_many_arguments)]
pub async fn get_line_chart_data<Resolution>(
    db: &DatabaseConnection,
    chart_name: &String,
    from: Option<Resolution>,
    to: Option<Resolution>,
    point_limit: Option<RequestedPointsLimit>,
    policy: MissingDatePolicy,
    fill_missing_dates: bool,
    approximate_trailing_points: u64,
) -> Result<Vec<ExtendedTimespanValue<Resolution, String>>, ReadError>
where
    Resolution: Timespan + ApproxUnsignedDiff + Debug + Ord + Clone,
{
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(
            charts::Column::Name.eq(chart_name).and(
                charts::Column::Resolution.eq(ChartResolution::from(Resolution::enum_variant())),
            ),
        )
        .one(db)
        .await?
        .ok_or_else(|| {
            ReadError::ChartNotFound(ChartKey::new(chart_name.into(), Resolution::enum_variant()))
        })?;

    // may contain points outside the range
    let db_data =
        get_raw_line_chart_data::<Resolution>(db, chart.id, from.clone(), to.clone()).await?;

    let (relevant_until, decrement_approx_points) = relevant_data_until(chart.last_updated_at);
    if relevant_until.is_none() && !db_data.is_empty() {
        tracing::warn!(
            chart_name = chart.name,
            db_data_len = db_data.len(),
            "`last_updated_at` is not set whereas data is present in DB"
        );
    }

    // If `to` is `None`, we would like to return all known points.
    // However, if some points are omitted, they should be filled according
    // to policy.
    // This fill makes sense up to the latest update.
    let to = match (to.clone(), relevant_until.clone()) {
        (Some(to), Some(relevant_until)) => Some(to.min(relevant_until)),
        (None, Some(relevant_until)) => Some(relevant_until),
        // It means `last_updated_at=None`, so `to` is set to `None` so the end is
        // later deduced from `db_data`.
        // It will return data that we have (if any, for some weird reason), or nothing
        // if the data is empty (=chart is new)
        (Some(_), None) => None,
        (None, None) => None,
    };

    let data_in_range = fit_into_range(db_data, from.clone(), to.clone(), policy);

    let data_unmarked = if fill_missing_dates {
        fill_and_filter_chart(data_in_range, from, to, policy, point_limit)?
    } else {
        data_in_range
    };
    let mut approximate_trailing_points = approximate_trailing_points;
    if decrement_approx_points {
        approximate_trailing_points = approximate_trailing_points.saturating_sub(1)
    }
    let data = mark_approximate(
        data_unmarked,
        relevant_until.unwrap_or(Resolution::from_date(NaiveDate::MAX)),
        approximate_trailing_points,
    );
    Ok(data)
}

/// Get data points at least within the provided range.
///
/// I.e. if today there was no data, but `from` is today, yesterday's
/// point will be also retrieved.
async fn get_raw_line_chart_data<Resolution>(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<Resolution>,
    to: Option<Resolution>,
) -> Result<Vec<TimespanValue<Resolution, String>>, DbErr>
where
    Resolution: Timespan,
{
    let mut data_request = chart_data::Entity::find()
        .column(chart_data::Column::Date)
        .column(chart_data::Column::Value)
        .filter(chart_data::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data::Column::Date);

    if let Some(from) = from {
        let from = from.into_date();
        let custom_where = Expr::cust_with_values::<_, sea_orm::sea_query::Value, _>(
            "date >= (SELECT COALESCE(MAX(date), '1900-01-01'::date) FROM chart_data WHERE chart_id = $1 AND date <= $2)",
            [chart_id.into(), from.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    }
    if let Some(to) = to {
        let to = to.into_date();
        let custom_where = Expr::cust_with_values::<_, sea_orm::sea_query::Value, _>(
            "date <= (SELECT COALESCE(MIN(date), '9999-12-31'::date) FROM chart_data WHERE chart_id = $1 AND date >= $2)",
            [chart_id.into(), to.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    };

    let data: Vec<DateValue<String>> = data_request.into_model().all(db).await?;
    let data = data
        .into_iter()
        .map(
            |TimespanValue {
                 timespan: date,
                 value,
             }| TimespanValue {
                timespan: Resolution::from_date(date),
                value,
            },
        )
        .collect_vec();
    Ok(data)
}

pub trait ApproxUnsignedDiff {
    /// Approx number of repeats of this timespan to get from `other` to `self`.
    ///
    /// `None` if < 0.
    fn approx_unsigned_difference(&self, other: &Self) -> Option<u64>;
}

impl ApproxUnsignedDiff for NaiveDate {
    fn approx_unsigned_difference(&self, other: &Self) -> Option<u64> {
        self.signed_duration_since(*other)
            .num_days()
            .try_into()
            .ok()
    }
}

impl ApproxUnsignedDiff for Week {
    fn approx_unsigned_difference(&self, other: &Self) -> Option<u64> {
        self.saturating_first_day()
            .signed_duration_since(other.saturating_first_day())
            .num_days()
            .try_into()
            .ok()
            .map(|n: u64| n / 7)
    }
}

impl ApproxUnsignedDiff for Month {
    fn approx_unsigned_difference(&self, other: &Self) -> Option<u64> {
        self.saturating_first_day()
            .signed_duration_since(other.saturating_first_day())
            .num_days()
            .try_into()
            .ok()
            // 30.436875 = mean # of days in month (according to wiki)
            .map(|n: u64| (n as f64 / 30.436875) as u64)
    }
}

impl ApproxUnsignedDiff for Year {
    fn approx_unsigned_difference(&self, other: &Self) -> Option<u64> {
        self.number_within_naive_date()
            .saturating_sub(other.number_within_naive_date())
            .try_into()
            .ok()
    }
}

#[derive(Debug, FromQueryResult)]
struct SyncInfo {
    pub min_blockscout_block: Option<i64>,
}

/// Get last 'finalized' row (for which no recomputations needed).
///
/// In case of inconsistencies or set `force_full`, returns `None`.
#[instrument(level="info", skip_all, fields(min_indexer_block = observed_min_indexer_block, chart =? ChartProps::key()))]
pub async fn last_accurate_point<ChartProps, Query>(
    chart_id: i32,
    observed_min_indexer_block: i64,
    db: &DatabaseConnection,
    force_full: bool,
    approximate_trailing_points: u64,
    policy: MissingDatePolicy,
) -> Result<Option<TimespanValue<ChartProps::Resolution, String>>, ChartError>
where
    ChartProps: ChartProperties + ?Sized,
    ChartProps::Resolution: Ord + Clone + Debug,
    Query: QueryBehaviour,
{
    let row = if force_full {
        tracing::info!("running full update due to force override");
        None
    } else {
        let recorded_min_indexer_block: Option<SyncInfo> = chart_data::Entity::find()
            .select_only()
            .column(chart_data::Column::MinBlockscoutBlock)
            .filter(chart_data::Column::ChartId.eq(chart_id))
            .order_by_desc(chart_data::Column::Date)
            .into_model()
            .one(db)
            .await
            .map_err(ChartError::StatsDB)?;

        match recorded_min_indexer_block {
            Some(SyncInfo {
                min_blockscout_block: Some(recorded_min_indexer_block),
            }) if recorded_min_indexer_block == observed_min_indexer_block => {
                let metadata = get_chart_metadata(db, &ChartProps::key()).await?;
                let Some(last_updated_at) = metadata.last_updated_at else {
                    // data is present, but `last_updated_at` is not set
                    tracing::info!("running full update due to lack of last_updated_at");
                    return Ok(None);
                };
                let last_updated_timespan =
                    ChartProps::Resolution::from_date(last_updated_at.date_naive());

                match ChartProps::chart_type() {
                    ChartType::Counter => {
                        // `approximate_trailing_points` doesn't really make sense for counters
                        // so it remains unused
                        let data = get_counter_data(
                            db,
                            &ChartProps::name(),
                            Some(last_updated_timespan.into_date()),
                            policy,
                        )
                        .await?;
                        data.map(|d| TimespanValue {
                            timespan: ChartProps::Resolution::from_date(d.timespan),
                            value: d.value,
                        })
                    }
                    ChartType::Line => {
                        let data = get_line_chart_data::<ChartProps::Resolution>(
                            db,
                            &ChartProps::name(),
                            Some(last_updated_timespan.saturating_sub(
                                TimespanDuration::from_timespan_repeats(
                                    approximate_trailing_points,
                                ),
                            )),
                            Some(last_updated_timespan),
                            None,
                            policy,
                            true,
                            approximate_trailing_points,
                        )
                        .await?;
                        let last_accurate_point = data.into_iter().rev().find_map(|p| {
                            if p.is_approximate {
                                None
                            } else {
                                Some(TimespanValue {
                                    timespan: p.timespan,
                                    value: p.value,
                                })
                            }
                        });
                        let Some(last_accurate_point) = last_accurate_point else {
                            return Err(ChartError::Internal("Failure while reading chart data: did not return accurate data (with `fill_missing_dates`=true)".into()));
                        };

                        tracing::info!(
                            min_chart_block = recorded_min_indexer_block,
                            last_accurate_point = ?last_accurate_point,
                            "running partial update"
                        );
                        Some(last_accurate_point)
                    }
                }
            }
            // != min_indexer_block
            Some(SyncInfo {
                min_blockscout_block: Some(block),
            }) => {
                tracing::info!(
                    min_chart_block = block,
                    "running full update due to min blocks mismatch"
                );
                None
            }
            Some(SyncInfo {
                min_blockscout_block: None,
            }) => {
                tracing::info!("running full update due to lack of saved min block");
                None
            }
            None => {
                tracing::info!("running full update due to lack of history data");
                None
            }
        }
    };

    Ok(row)
}

/// May not be exact, but the limit is close to
/// this number
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RequestedPointsLimit {
    Points(u32),
    NoLimit,
}

impl RequestedPointsLimit {
    pub fn from_points(approx_limit: u32) -> Self {
        Self::Points(approx_limit)
    }

    pub fn approx_limit(&self) -> Option<u32> {
        match self {
            RequestedPointsLimit::Points(p) => Some(*p),
            RequestedPointsLimit::NoLimit => None,
        }
    }

    pub fn fits_in_limit<T: ApproxUnsignedDiff>(&self, from: &T, to: &T) -> bool {
        let limit = match self {
            RequestedPointsLimit::Points(p) => *p,
            RequestedPointsLimit::NoLimit => return true,
        };
        to.approx_unsigned_difference(from)
            .map(|diff| diff <= limit.into())
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Named,
        charts::ResolutionKind,
        counters::TotalBlocks,
        data_source::{
            DataSource, UpdateContext, UpdateParameters,
            kinds::local_db::parameters::DefaultQueryVec, types::IndexerMigrations,
        },
        lines::{AccountsGrowth, ActiveAccounts, NewTxns, TxnsGrowth, TxnsGrowthMonthly},
        tests::{
            init_db::init_db,
            point_construction::{d, dt, month_of},
            simple_test::get_counter,
        },
        types::timespans::Month,
    };
    use chrono::DateTime;
    use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
    use pretty_assertions::assert_eq;
    use sea_orm::{EntityTrait, Set};
    use std::{collections::HashMap, str::FromStr};

    fn mock_chart_data(chart_id: i32, date: &str, value: i64) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            chart_id: Set(chart_id),
            date: Set(NaiveDate::from_str(date).unwrap()),
            value: Set(value.to_string()),
            min_blockscout_block: Set(Some(1)),
            ..Default::default()
        }
    }

    async fn insert_mock_data(db: &DatabaseConnection) -> HashMap<ChartKey, i32> {
        let charts = [
            charts::ActiveModel {
                name: Set(NewTxns::name().to_string()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-12T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newBlocksPerDay".into()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-12T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newVerifiedContracts".into()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-15T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set(TxnsGrowth::name().to_string()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-30T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set(TxnsGrowth::name().to_string()),
                resolution: Set(ChartResolution::Month),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-30T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            // the chart was only created
            charts::ActiveModel {
                name: Set(AccountsGrowth::name().to_string()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(None),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set(TotalBlocks::name().to_string()),
                resolution: Set(ChartResolution::Day),
                chart_type: Set(ChartType::Counter),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-12T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
        ];
        let chart_ids: HashMap<_, _> = charts
            .iter()
            .enumerate()
            .map(|(i, c)| {
                (
                    ChartKey::new(
                        c.name.clone().unwrap(),
                        c.resolution.clone().unwrap().into(),
                    ),
                    (i + 1) as i32,
                )
            })
            .collect();
        charts::Entity::insert_many(charts).exec(db).await.unwrap();
        chart_data::Entity::insert_many([
            mock_chart_data(
                chart_ids[&ChartKey::with_day(NewTxns::name())],
                "2022-11-10",
                1000,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(NewTxns::name())],
                "2022-11-11",
                1150,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(NewTxns::name())],
                "2022-11-12",
                1350,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day("newBlocksPerDay".into())],
                "2022-11-10",
                100,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day("newBlocksPerDay".into())],
                "2022-11-11",
                150,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day("newBlocksPerDay".into())],
                "2022-11-12",
                200,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day("newVerifiedContracts".into())],
                "2022-11-13",
                2,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day("newVerifiedContracts".into())],
                "2022-11-15",
                3,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TxnsGrowth::name())],
                "2022-11-17",
                123,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TxnsGrowth::name())],
                "2022-11-19",
                323,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TxnsGrowth::name())],
                "2022-11-29",
                1000,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::new(TxnsGrowth::name(), ResolutionKind::Month)],
                "2022-08-17",
                12,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::new(TxnsGrowth::name(), ResolutionKind::Month)],
                "2022-09-19",
                100,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::new(TxnsGrowth::name(), ResolutionKind::Month)],
                "2022-10-29",
                1000,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TotalBlocks::name())],
                "2022-08-17",
                12,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TotalBlocks::name())],
                "2022-09-19",
                100,
            ),
            mock_chart_data(
                chart_ids[&ChartKey::with_day(TotalBlocks::name())],
                "2022-10-29",
                1000,
            ),
        ])
        .exec(db)
        .await
        .unwrap();
        chart_ids
    }

    /// Depicts a chart during the process of reupdate
    async fn insert_mock_data_reupdate(db: &DatabaseConnection) {
        charts::Entity::insert_many([charts::ActiveModel {
            name: Set(ActiveAccounts::name().to_string()),
            resolution: Set(ChartResolution::Day),
            chart_type: Set(ChartType::Line),
            last_updated_at: Set(Some(
                DateTime::parse_from_rfc3339("2022-06-30T00:00:00+00:00").unwrap(),
            )),
            ..Default::default()
        }])
        .exec(db)
        .await
        .unwrap();
        chart_data::Entity::insert_many([
            mock_chart_data(1, "2022-06-19", 74),
            mock_chart_data(1, "2022-06-20", 103),
            mock_chart_data(1, "2022-06-21", 199),
            mock_chart_data(1, "2022-06-22", 94),
            mock_chart_data(1, "2022-06-23", 123),
            mock_chart_data(1, "2022-06-24", 277),
            mock_chart_data(1, "2022-06-25", 34),
            mock_chart_data(1, "2022-06-26", 51),
            mock_chart_data(1, "2022-06-27", 81),
            mock_chart_data(1, "2022-06-28", 57),
            mock_chart_data(1, "2022-06-29", 1676),
            mock_chart_data(1, "2022-06-30", 1636),
            mock_chart_data(1, "2022-07-01", 99),
            mock_chart_data(1, "2022-07-02", 1585),
        ])
        .exec(db)
        .await
        .unwrap();
    }

    fn value(date: &str, value: &str) -> DateValue<String> {
        DateValue::<String> {
            timespan: NaiveDate::from_str(date).unwrap(),
            value: value.to_string(),
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_counter_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_counter_mock").await;
        insert_mock_data(&db).await;
        let current_time = dt("2022-11-12T08:08:08").and_utc();
        let date = current_time.date_naive();
        let cx = UpdateContext::from_params_now_or_override(UpdateParameters {
            stats_db: &db,
            is_multichain_mode: false,
            // shouldn't use this because mock data contains total blocks value
            indexer_db: &db,
            indexer_applied_migrations: IndexerMigrations::latest(),
            enabled_update_charts_recursive: TotalBlocks::all_dependencies_chart_keys(),
            update_time_override: Some(current_time),
            force_full: false,
        });
        assert_eq!(
            value(&date.to_string(), "1000"),
            get_counter::<TotalBlocks>(&cx).await
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_int_mock").await;
        insert_mock_data(&db).await;
        let data = get_line_chart_data::<NaiveDate>(
            &db,
            &"newBlocksPerDay".to_string(),
            None,
            None,
            None,
            MissingDatePolicy::FillZero,
            false,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-10").unwrap(),
                    value: "100".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-11").unwrap(),
                    value: "150".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-12").unwrap(),
                    value: "200".into(),
                    is_approximate: true,
                },
            ],
            data
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_monthly() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_int_monthly").await;
        insert_mock_data(&db).await;
        let data = get_line_chart_data::<Month>(
            &db,
            &TxnsGrowth::name(),
            None,
            None,
            None,
            MissingDatePolicy::FillPrevious,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: month_of("2022-08-01"),
                    value: "12".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-09-01"),
                    value: "100".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-10-01"),
                    value: "1000".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-11-01"),
                    value: "1000".into(),
                    is_approximate: true,
                },
            ],
            data
        );

        let data = get_line_chart_data::<Month>(
            &db,
            &TxnsGrowth::name(),
            None,
            None,
            None,
            MissingDatePolicy::FillZero,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: month_of("2022-08-01"),
                    value: "12".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-09-01"),
                    value: "100".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-10-01"),
                    value: "1000".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: month_of("2022-11-01"),
                    value: "0".into(),
                    is_approximate: true,
                },
            ],
            data
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_data_skipped_works() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_data_skipped_works").await;
        insert_mock_data(&db).await;
        let data = get_line_chart_data::<NaiveDate>(
            &db,
            &"newVerifiedContracts".to_string(),
            Some(d("2022-11-14")),
            Some(d("2022-11-15")),
            None,
            MissingDatePolicy::FillPrevious,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-14").unwrap(),
                    value: "2".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-15").unwrap(),
                    value: "3".into(),
                    is_approximate: true,
                },
            ],
            data
        );

        let data = get_line_chart_data::<NaiveDate>(
            &db,
            &"newVerifiedContracts".to_string(),
            Some(d("2022-11-12")),
            Some(d("2022-11-14")),
            None,
            MissingDatePolicy::FillPrevious,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-12").unwrap(),
                    value: "0".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-13").unwrap(),
                    value: "2".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-14").unwrap(),
                    value: "2".into(),
                    is_approximate: false,
                },
            ],
            data
        );

        let data = get_line_chart_data::<NaiveDate>(
            &db,
            &TxnsGrowth::name(),
            Some(d("2022-11-28")),
            Some(d("2022-11-30")),
            None,
            MissingDatePolicy::FillPrevious,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            vec![
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-28").unwrap(),
                    value: "323".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-29").unwrap(),
                    value: "1000".into(),
                    is_approximate: false,
                },
                ExtendedTimespanValue {
                    timespan: NaiveDate::from_str("2022-11-30").unwrap(),
                    value: "1000".into(),
                    is_approximate: true,
                },
            ],
            data
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_new_chart_data_returns_nothing() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_new_chart_data_returns_nothing").await;
        insert_mock_data(&db).await;
        let data = get_line_chart_data::<NaiveDate>(
            &db,
            &AccountsGrowth::name().to_string(),
            Some(d("2022-11-14")),
            Some(d("2022-11-15")),
            None,
            MissingDatePolicy::FillPrevious,
            true,
            1,
        )
        .await
        .unwrap();
        assert_eq!(data, vec![]);
    }

    async fn chart_id_matches_key(
        db: &DatabaseConnection,
        chart_id: i32,
        name: &str,
        resolution: ResolutionKind,
    ) -> bool {
        let metadata = get_chart_metadata(db, &ChartKey::new(name.into(), resolution))
            .await
            .unwrap();
        metadata.id == chart_id
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn last_accurate_point_daily_works() {
        let _ = tracing_subscriber::fmt::try_init();
        let db = init_db("last_accurate_point_daily_works").await;
        insert_mock_data(&db).await;

        // No missing points
        assert!(chart_id_matches_key(&db, 1, "newTxns", ResolutionKind::Day).await);
        assert_eq!(
            last_accurate_point::<NewTxns, DefaultQueryVec<NewTxns>>(
                1,
                1,
                &db,
                false,
                0,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-12"),
                value: "1350".to_string()
            })
        );
        assert_eq!(
            last_accurate_point::<NewTxns, DefaultQueryVec<NewTxns>>(
                1,
                1,
                &db,
                false,
                1,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-11"),
                value: "1150".to_string()
            })
        );
        assert_eq!(
            last_accurate_point::<NewTxns, DefaultQueryVec<NewTxns>>(
                1,
                1,
                &db,
                false,
                2,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-10"),
                value: "1000".to_string()
            })
        );

        // Missing points
        assert!(chart_id_matches_key(&db, 4, &TxnsGrowth::name(), ResolutionKind::Day).await);
        assert_eq!(
            last_accurate_point::<TxnsGrowth, DefaultQueryVec<TxnsGrowth>>(
                4,
                1,
                &db,
                false,
                0,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-30"),
                value: "1000".to_string()
            })
        );
        assert_eq!(
            last_accurate_point::<TxnsGrowth, DefaultQueryVec<TxnsGrowth>>(
                4,
                1,
                &db,
                false,
                1,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-29"),
                value: "1000".to_string()
            })
        );
        assert_eq!(
            last_accurate_point::<TxnsGrowth, DefaultQueryVec<TxnsGrowth>>(
                4,
                1,
                &db,
                false,
                2,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-11-28"),
                value: "323".to_string()
            })
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn last_accurate_point_monthly_works() {
        let _ = tracing_subscriber::fmt::try_init();
        let db = init_db("last_accurate_point_monthly_works").await;
        insert_mock_data(&db).await;

        assert!(chart_id_matches_key(&db, 5, &TxnsGrowth::name(), ResolutionKind::Month).await);
        assert_eq!(
            last_accurate_point::<TxnsGrowthMonthly, DefaultQueryVec<TxnsGrowthMonthly>>(
                5,
                1,
                &db,
                false,
                1,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(TimespanValue {
                timespan: month_of("2022-10-01"),
                value: "1000".into(),
            }),
        );
        assert_eq!(
            last_accurate_point::<TxnsGrowthMonthly, DefaultQueryVec<TxnsGrowthMonthly>>(
                5,
                1,
                &db,
                false,
                0,
                MissingDatePolicy::FillPrevious
            )
            .await
            .unwrap(),
            Some(TimespanValue {
                timespan: month_of("2022-11-01"),
                value: "1000".into(),
            }),
        );
        assert_eq!(
            last_accurate_point::<TxnsGrowthMonthly, DefaultQueryVec<TxnsGrowthMonthly>>(
                5,
                1,
                &db,
                false,
                0,
                MissingDatePolicy::FillZero
            )
            .await
            .unwrap(),
            Some(TimespanValue {
                timespan: month_of("2022-11-01"),
                value: "0".into(),
            }),
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn last_accurate_point_reupdate_works() {
        let _ = tracing_subscriber::fmt::try_init();
        let db = init_db("last_accurate_point_2_works").await;
        insert_mock_data_reupdate(&db).await;

        // No missing points
        assert!(chart_id_matches_key(&db, 1, "activeAccounts", ResolutionKind::Day).await);
        assert_eq!(
            last_accurate_point::<ActiveAccounts, DefaultQueryVec<ActiveAccounts>>(
                1,
                1,
                &db,
                false,
                1,
                MissingDatePolicy::FillZero
            )
            .await
            .unwrap(),
            Some(DateValue::<String> {
                timespan: d("2022-06-29"),
                value: "1676".to_string()
            })
        );
    }
}
