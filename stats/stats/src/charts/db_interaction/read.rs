use crate::{
    charts::{chart::ChartMetadata, db_interaction::types::ZeroDateValue},
    missing_date::{fill_and_filter_chart, fit_into_range},
    ChartProperties, DateValueString, ExtendedDateValue, MissingDatePolicy, UpdateError,
};

use blockscout_db::entity::blocks;
use chrono::{Duration, NaiveDate, NaiveDateTime, Utc};
use entity::{chart_data, charts};
use sea_orm::{
    sea_query::{self, Expr},
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait,
    FromQueryResult, QueryFilter, QueryOrder, QuerySelect, Statement,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ReadError {
    #[error("database error {0}")]
    DB(#[from] DbErr),
    #[error("chart {0} not found")]
    ChartNotFound(String),
    #[error("date interval limit ({0}) is exceeded; choose smaller time interval.")]
    IntervalLimitExceeded(Duration),
}

#[derive(Debug, FromQueryResult)]
struct ChartID {
    id: i32,
}

pub async fn find_chart(db: &DatabaseConnection, name: &str) -> Result<Option<i32>, DbErr> {
    charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .into_model::<ChartID>()
        .one(db)
        .await
        .map(|id| id.map(|id| id.id))
}

#[derive(Debug, FromQueryResult)]
struct CounterData {
    name: String,
    date: NaiveDate,
    value: String,
}

/// Get counters with raw values
/// (i.e. with date relevance not handled)
pub async fn get_raw_counters(
    db: &DatabaseConnection,
) -> Result<HashMap<String, DateValueString>, ReadError> {
    let data = CounterData::find_by_statement(Statement::from_string(
        DbBackend::Postgres,
        r#"
            SELECT distinct on (charts.id) charts.name, data.date, data.value
            FROM "chart_data" "data"
            INNER JOIN "charts"
                ON data.chart_id = charts.id
            WHERE charts.chart_type = 'COUNTER'
            ORDER BY charts.id, data.id DESC;
        "#,
    ))
    .all(db)
    .await?;

    let counters: HashMap<_, _> = data
        .into_iter()
        .map(|data| {
            (
                data.name,
                DateValueString {
                    date: data.date,
                    value: data.value,
                },
            )
        })
        .collect();

    Ok(counters)
}

/// Get counter value for the requested date
pub async fn get_counter_data(
    db: &DatabaseConnection,
    name: &str,
    date: Option<NaiveDate>,
    policy: MissingDatePolicy,
) -> Result<Option<DateValueString>, ReadError> {
    let current_date = date.unwrap_or(Utc::now().date_naive());
    let raw_data = DateValueString::find_by_statement(Statement::from_sql_and_values(
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
            p.date = current_date;
            p
        }
    });
    Ok(data)
}

/// Mark corresponding data points as approximate.
///
/// Approximate are:
/// - points after `last_updated_at` date
/// - `approximate_until_updated` dates starting from `last_updated_at` and moving back
///
/// If `approximate_until_updated=0` - only future points are marked
fn mark_approximate(
    data: Vec<DateValueString>,
    last_updated_at: NaiveDate,
    approximate_until_updated: u64,
) -> Vec<ExtendedDateValue> {
    // saturating sub/add
    let next_after_updated_at = last_updated_at
        .checked_add_days(chrono::Days::new(1))
        .unwrap_or(NaiveDate::MAX);
    let mark_from_date = next_after_updated_at
        .checked_sub_days(chrono::Days::new(approximate_until_updated))
        .unwrap_or(NaiveDate::MIN);
    data.into_iter()
        .map(|dv| {
            let is_marked = dv.date >= mark_from_date;
            ExtendedDateValue::from_date_value(dv, is_marked)
        })
        .collect()
}

pub async fn get_chart_metadata(
    db: &DatabaseConnection,
    name: &str,
) -> Result<ChartMetadata, ReadError> {
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or_else(|| ReadError::ChartNotFound(name.into()))?;
    Ok(ChartMetadata {
        last_updated_at: chart.last_updated_at.map(|t| t.with_timezone(&Utc)),
        id: chart.id,
        created_at: chart.created_at.with_timezone(&Utc),
    })
}

/// Get data points for the chart `name`.
///
/// `approximate_trailing_points` - number of trailing points to mark as approximate.
///
/// `interval_limit` - max interval (from, to). If `from` or `to` are none,
/// min or max date in DB are calculated.
///
/// Note: if some dates within interval `(from, to)` fall on the future, no data points
/// for them are returned.
#[allow(clippy::too_many_arguments)]
pub async fn get_line_chart_data(
    db: &DatabaseConnection,
    name: &str,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
    interval_limit: Option<Duration>,
    policy: MissingDatePolicy,
    fill_missing_dates: bool,
    approximate_trailing_points: u64,
) -> Result<Vec<ExtendedDateValue>, ReadError> {
    let chart = charts::Entity::find()
        .column(charts::Column::Id)
        .filter(charts::Column::Name.eq(name))
        .one(db)
        .await?
        .ok_or_else(|| ReadError::ChartNotFound(name.into()))?;

    // may contain points outside the range, need to figure it out
    let db_data = get_raw_line_chart_data(db, chart.id, from, to).await?;

    let last_updated_at = chart.last_updated_at.map(|t| t.date_naive());
    if last_updated_at.is_none() && !db_data.is_empty() {
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
    let to = match (to, last_updated_at) {
        (Some(to), Some(last_updated_at)) => Some(to.min(last_updated_at)),
        (None, Some(d)) | (Some(d), None) => Some(d),
        (None, None) => None,
    };

    let data_in_range = fit_into_range(db_data, from, to, policy);

    let data_unmarked = if fill_missing_dates {
        fill_and_filter_chart(data_in_range, from, to, policy, interval_limit)?
    } else {
        data_in_range
    };
    let data = mark_approximate(
        data_unmarked,
        last_updated_at.unwrap_or(NaiveDate::MAX),
        approximate_trailing_points,
    );
    Ok(data)
}

/// Get data points at least within the provided range.
///
/// I.e. if today there was no data, but `from` is today, yesterday's
/// point will be also retrieved.
async fn get_raw_line_chart_data(
    db: &DatabaseConnection,
    chart_id: i32,
    from: Option<NaiveDate>,
    to: Option<NaiveDate>,
) -> Result<Vec<DateValueString>, DbErr> {
    let mut data_request = chart_data::Entity::find()
        .column(chart_data::Column::Date)
        .column(chart_data::Column::Value)
        .filter(chart_data::Column::ChartId.eq(chart_id))
        .order_by_asc(chart_data::Column::Date);

    if let Some(from) = from {
        let custom_where = Expr::cust_with_values::<_, sea_orm::sea_query::Value, _>(
            "date >= (SELECT COALESCE(MAX(date), '1900-01-01'::date) FROM chart_data WHERE chart_id = $1 AND date <= $2)",
            [chart_id.into(), from.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    }
    if let Some(to) = to {
        let custom_where = Expr::cust_with_values::<_, sea_orm::sea_query::Value, _>(
            "date <= (SELECT COALESCE(MIN(date), '9999-12-31'::date) FROM chart_data WHERE chart_id = $1 AND date >= $2)",
            [chart_id.into(), to.into()],
        );
        QuerySelect::query(&mut data_request).cond_where(custom_where);
    };

    let data = data_request.into_model().all(db).await?;
    Ok(data)
}

#[derive(FromQueryResult)]
struct MinBlock {
    min_block: i64,
}

pub async fn get_min_block_blockscout(blockscout: &DatabaseConnection) -> Result<i64, DbErr> {
    let min_block = blocks::Entity::find()
        .select_only()
        .column_as(
            sea_query::Expr::col(blocks::Column::Number).min(),
            "min_block",
        )
        .filter(blocks::Column::Consensus.eq(true))
        .into_model::<MinBlock>()
        .one(blockscout)
        .await?;

    min_block
        .map(|r| r.min_block)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}

#[derive(FromQueryResult)]
struct MinDate {
    timestamp: NaiveDateTime,
}

pub async fn get_min_date_blockscout<C>(blockscout: &C) -> Result<NaiveDateTime, DbErr>
where
    C: ConnectionTrait,
{
    let min_date = blocks::Entity::find()
        .select_only()
        .column(blocks::Column::Timestamp)
        .filter(blocks::Column::Consensus.eq(true))
        // First block on ethereum mainnet has 0 timestamp,
        // however first block on Goerli for example has valid timestamp.
        // Therefore we filter on zero timestamp
        .filter(blocks::Column::Timestamp.ne(NaiveDateTime::default()))
        .order_by_asc(blocks::Column::Number)
        .into_model::<MinDate>()
        .one(blockscout)
        .await?;

    min_date
        .map(|r| r.timestamp)
        .ok_or_else(|| DbErr::RecordNotFound("no blocks found in blockscout database".into()))
}

#[derive(Debug, FromQueryResult)]
struct SyncInfo {
    pub date: NaiveDate,
    pub value: String,
    pub min_blockscout_block: Option<i64>,
}

/// Get last 'finalized' row (for which no recomputations needed).
///
/// Retrieves `offset`th latest data point from DB, if any.
/// In case of inconsistencies or set `force_full`, also returns `None`.
pub async fn last_accurate_point<C>(
    chart_id: i32,
    min_blockscout_block: i64,
    db: &DatabaseConnection,
    force_full: bool,
    offset: Option<u64>,
) -> Result<Option<DateValueString>, UpdateError>
where
    C: ChartProperties + ?Sized,
{
    let offset = offset.unwrap_or(0);
    let row = if force_full {
        tracing::info!(
            min_blockscout_block = min_blockscout_block,
            chart = C::NAME,
            "running full update due to force override"
        );
        None
    } else {
        let row: Option<SyncInfo> = chart_data::Entity::find()
            .column(chart_data::Column::Date)
            .column(chart_data::Column::Value)
            .column(chart_data::Column::MinBlockscoutBlock)
            .filter(chart_data::Column::ChartId.eq(chart_id))
            .order_by_desc(chart_data::Column::Date)
            .offset(offset)
            .into_model()
            .one(db)
            .await
            .map_err(UpdateError::StatsDB)?;

        match row {
            Some(row) => {
                if let Some(block) = row.min_blockscout_block {
                    if block == min_blockscout_block {
                        tracing::info!(
                            min_blockscout_block = min_blockscout_block,
                            min_chart_block = block,
                            chart = C::NAME,
                            row = ?row,
                            "running partial update"
                        );
                        Some(DateValueString {
                            date: row.date,
                            value: row.value,
                        })
                    } else {
                        tracing::info!(
                            min_blockscout_block = min_blockscout_block,
                            min_chart_block = block,
                            chart = C::NAME,
                            "running full update due to min blocks mismatch"
                        );
                        None
                    }
                } else {
                    tracing::info!(
                        min_blockscout_block = min_blockscout_block,
                        chart = C::NAME,
                        "running full update due to lack of saved min block"
                    );
                    None
                }
            }
            None => {
                tracing::info!(
                    min_blockscout_block = min_blockscout_block,
                    chart = C::NAME,
                    "running full update due to lack of history data"
                );
                None
            }
        }
    };

    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        counters::TotalBlocks,
        tests::{init_db::init_db, point_construction::d},
        Named,
    };
    use chrono::DateTime;
    use entity::{chart_data, charts, sea_orm_active_enums::ChartType};
    use pretty_assertions::assert_eq;
    use sea_orm::{EntityTrait, Set};
    use std::str::FromStr;

    fn mock_chart_data(chart_id: i32, date: &str, value: i64) -> chart_data::ActiveModel {
        chart_data::ActiveModel {
            chart_id: Set(chart_id),
            date: Set(NaiveDate::from_str(date).unwrap()),
            value: Set(value.to_string()),
            ..Default::default()
        }
    }

    async fn insert_mock_data(db: &DatabaseConnection) {
        charts::Entity::insert_many([
            charts::ActiveModel {
                name: Set(TotalBlocks::NAME.to_string()),
                chart_type: Set(ChartType::Counter),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-12T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newBlocksPerDay".into()),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-12T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
            charts::ActiveModel {
                name: Set("newVerifiedContracts".into()),
                chart_type: Set(ChartType::Line),
                last_updated_at: Set(Some(
                    DateTime::parse_from_rfc3339("2022-11-15T08:08:08+00:00").unwrap(),
                )),
                ..Default::default()
            },
        ])
        .exec(db)
        .await
        .unwrap();
        chart_data::Entity::insert_many([
            mock_chart_data(1, "2022-11-10", 1000),
            mock_chart_data(2, "2022-11-10", 100),
            mock_chart_data(1, "2022-11-11", 1150),
            mock_chart_data(2, "2022-11-11", 150),
            mock_chart_data(1, "2022-11-12", 1350),
            mock_chart_data(2, "2022-11-12", 200),
            mock_chart_data(3, "2022-11-13", 2),
            mock_chart_data(3, "2022-11-15", 3),
        ])
        .exec(db)
        .await
        .unwrap();
    }

    fn value(date: &str, value: &str) -> DateValueString {
        DateValueString {
            date: NaiveDate::from_str(date).unwrap(),
            value: value.to_string(),
        }
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_counters_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_counters_mock").await;
        insert_mock_data(&db).await;
        let counters = get_raw_counters(&db).await.unwrap();
        assert_eq!(
            HashMap::from_iter([("totalBlocks".into(), value("2022-11-12", "1350"))]),
            counters
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_int_mock() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_int_mock").await;
        insert_mock_data(&db).await;
        let chart = get_line_chart_data(
            &db,
            "newBlocksPerDay",
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
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-10").unwrap(),
                    value: "100".into(),
                    is_approximate: false,
                },
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-11").unwrap(),
                    value: "150".into(),
                    is_approximate: false,
                },
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-12").unwrap(),
                    value: "200".into(),
                    is_approximate: true,
                },
            ],
            chart
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn get_chart_data_skipped_works() {
        let _ = tracing_subscriber::fmt::try_init();

        let db = init_db("get_chart_data_skipped_works").await;
        insert_mock_data(&db).await;
        let chart = get_line_chart_data(
            &db,
            "newVerifiedContracts",
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
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-14").unwrap(),
                    value: "2".into(),
                    is_approximate: false,
                },
                ExtendedDateValue {
                    date: NaiveDate::from_str("2022-11-15").unwrap(),
                    value: "3".into(),
                    is_approximate: true,
                },
            ],
            chart
        );
    }
}
