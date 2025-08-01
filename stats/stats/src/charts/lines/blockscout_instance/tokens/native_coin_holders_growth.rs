use std::collections::{BTreeMap, HashSet};

use crate::{
    ChartError, ChartProperties, MissingDatePolicy, Named,
    charts::db_interaction::write::{create_chart, insert_data_many},
    data_source::{
        UpdateContext,
        kinds::{
            data_manipulation::{
                map::{MapParseTo, StripExt},
                resolutions::last_value::LastValueLowerResolution,
            },
            local_db::{
                DirectVecLocalDbChartSource, LocalDbChartSource,
                parameter_traits::{CreateBehaviour, UpdateBehaviour},
                parameters::{
                    DefaultQueryVec,
                    update::batching::parameters::{Batch30Weeks, Batch30Years, Batch36Months},
                },
            },
        },
    },
    define_and_impl_resolution_properties,
    types::timespans::{DateValue, Month, Week, Year},
};

use blockscout_db::entity::address_coin_balances_daily;
use blockscout_metrics_tools::AggregateTimer;
use chrono::{NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use itertools::Itertools;
use migration::OnConflict;
use sea_orm::{
    ConnectionTrait, FromQueryResult, QueryOrder, QuerySelect, Set, Statement, TransactionTrait,
    prelude::*,
};
mod db_address_balances {
    use sea_orm::prelude::*;

    // `nchg` is native_coin_holders_growth
    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
    #[sea_orm(table_name = "support_nchg_addresses_balances")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub address: Vec<u8>,
        pub balance: Decimal,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "nativeCoinHoldersGrowth".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
    fn missing_date_policy() -> MissingDatePolicy {
        MissingDatePolicy::FillPrevious
    }
    fn approximate_trailing_points() -> u64 {
        // support table contains information of actual last day
        0
    }
}

pub struct Create;

impl CreateBehaviour for Create {
    async fn create(
        db: &DatabaseConnection,
        init_time: &chrono::DateTime<Utc>,
    ) -> Result<(), DbErr> {
        create_support_table(db).await?;
        create_chart(db, Properties::key(), Properties::chart_type(), init_time).await
    }
}

pub struct Update;

impl UpdateBehaviour<(), (), NaiveDate> for Update {
    async fn update_values(
        cx: &UpdateContext<'_>,
        chart_id: i32,
        last_accurate_point: Option<DateValue<String>>,
        min_indexer_block: i64,
        dependency_data_fetch_timer: &mut AggregateTimer,
    ) -> Result<(), ChartError> {
        update_sequentially_with_support_table(
            cx,
            chart_id,
            last_accurate_point,
            min_indexer_block,
            dependency_data_fetch_timer,
        )
        .await
    }
}

// TODO: move common logic to new type implementing some parameter trait
pub async fn update_sequentially_with_support_table(
    cx: &UpdateContext<'_>,
    chart_id: i32,
    last_accurate_point: Option<DateValue<String>>,
    min_indexer_block: i64,
    remote_fetch_timer: &mut AggregateTimer,
) -> Result<(), ChartError> {
    tracing::info!(chart =% Properties::key(), "start sequential update");
    let all_days = match last_accurate_point {
        Some(last_row) => {
            get_unique_ordered_days(cx.indexer_db, Some(last_row.timespan), remote_fetch_timer)
                .await
                .map_err(ChartError::IndexerDB)?
        }
        None => {
            clear_support_table(cx.stats_db)
                .await
                .map_err(ChartError::IndexerDB)?;
            get_unique_ordered_days(cx.indexer_db, None, remote_fetch_timer)
                .await
                .map_err(ChartError::IndexerDB)?
        }
    };

    for days in all_days.chunks(Properties::step_duration_days()) {
        let first = days.first();
        let last = days.last();
        tracing::info!(
            len = days.len(),
            first = ?first,
            last = ?last,
            "start fetching data for days"
        );
        // NOTE: we update support table and chart data in one transaction
        // to support invariant that support table has information about last day in chart data
        let db_tx = cx.stats_db.begin().await.map_err(ChartError::StatsDB)?;
        let data: Vec<entity::chart_data::ActiveModel> =
            calculate_days_using_support_table(&db_tx, cx.indexer_db, days.iter().copied())
                .await
                .map_err(|e| ChartError::Internal(e.to_string()))?
                .into_iter()
                .map(|result| result.active_model(chart_id, Some(min_indexer_block)))
                .collect();
        insert_data_many(&db_tx, data)
            .await
            .map_err(ChartError::StatsDB)?;
        db_tx.commit().await.map_err(ChartError::StatsDB)?;
    }
    Ok(())
}

async fn calculate_days_using_support_table<C1, C2>(
    db: &C1,
    blockscout: &C2,
    days: impl IntoIterator<Item = NaiveDate>,
) -> Result<Vec<DateValue<String>>, ChartError>
where
    C1: ConnectionTrait,
    C2: ConnectionTrait,
{
    let mut result = vec![];
    let new_holders_by_date = get_holder_changes_by_date(blockscout, days)
        .await
        .map_err(|e| ChartError::Internal(format!("cannot get new holders: {e}")))?;

    for (date, holders) in new_holders_by_date {
        // this check shouldnt be triggered if data in blockscout is correct,
        // but just in case...
        let addresses = holders.iter().map(|h| &h.address).collect::<HashSet<_>>();
        if addresses.len() != holders.len() {
            tracing::error!(addresses = ?addresses, date = ?date, "duplicate addresses in holders");
            return Err(ChartError::Internal(
                "duplicate addresses in holders".to_string(),
            ));
        };
        let holders = holders
            .into_iter()
            .map(|holder| db_address_balances::ActiveModel {
                address: Set(holder.address),
                balance: Set(holder.balance),
            });

        update_current_holders(db, holders)
            .await
            .map_err(|e| ChartError::Internal(format!("cannot update holders: {e}")))?;
        let new_count = count_current_holders(db)
            .await
            .map_err(|e| ChartError::Internal(format!("cannot count holders: {e}")))?;
        result.push(DateValue::<String> {
            timespan: date,
            value: new_count.to_string(),
        });
    }
    Ok(result)
}

async fn get_holder_changes_by_date<C>(
    blockscout: &C,
    days: impl IntoIterator<Item = NaiveDate>,
) -> Result<BTreeMap<NaiveDate, Vec<db_address_balances::Model>>, DbErr>
where
    C: ConnectionTrait,
{
    let days = days.into_iter().collect::<Vec<_>>();
    let all_holders = {
        // use BTreeMap to prevent address duplicates due to several queries
        let mut all_rows: BTreeMap<Vec<u8>, address_coin_balances_daily::Model> = BTreeMap::new();
        let limit = Properties::max_rows_fetch_per_iteration();
        let mut offset = 0;
        loop {
            let rows = address_coin_balances_daily::Entity::find()
                .filter(address_coin_balances_daily::Column::Day.is_in(days.clone()))
                .order_by_asc(address_coin_balances_daily::Column::AddressHash)
                .limit(limit)
                .offset(offset)
                .all(blockscout)
                .await?;
            let n = rows.len() as u64;

            all_rows.extend(&mut rows.into_iter().map(|row| (row.address_hash.clone(), row)));
            if n < limit {
                break;
            }
            offset += n;
        }
        all_rows
    };
    let holders_grouped: BTreeMap<NaiveDate, Vec<db_address_balances::Model>> = all_holders
        .into_values()
        .map(|row| {
            (
                row.day,
                db_address_balances::Model {
                    address: row.address_hash,
                    balance: row.value.unwrap_or_default(),
                },
            )
        })
        .into_group_map()
        .into_iter()
        .collect();

    tracing::debug!(result =? holders_grouped, "result of get holders in days");
    Ok(holders_grouped)
}

async fn create_support_table(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let statement = Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        format!(
            r#"
                CREATE TABLE IF NOT EXISTS {} (
                    address BYTEA PRIMARY KEY,
                    balance NUMERIC(100,0) NOT NULL
                )
                "#,
            Properties::support_table_name()
        ),
    );
    db.execute(statement).await?;
    Ok(())
}

async fn clear_support_table(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    let statement = Statement::from_string(
        sea_orm::DatabaseBackend::Postgres,
        format!("DELETE FROM {}", Properties::support_table_name()),
    );
    db.execute(statement).await?;
    Ok(())
}

async fn count_current_holders<C>(db: &C) -> Result<u64, DbErr>
where
    C: ConnectionTrait,
{
    let count = db_address_balances::Entity::find()
        .filter(db_address_balances::Column::Balance.gte(Properties::min_balance_for_holders()))
        .count(db)
        .await?;
    Ok(count)
}

async fn update_current_holders<C>(
    db: &C,
    holders: impl IntoIterator<Item = db_address_balances::ActiveModel>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    let mut data = holders.into_iter().peekable();
    let take = Properties::max_rows_insert_per_iteration();
    while data.peek().is_some() {
        let chunk: Vec<_> = data.by_ref().take(take).collect();
        db_address_balances::Entity::insert_many(chunk)
            .on_conflict(
                OnConflict::column(db_address_balances::Column::Address)
                    .update_column(db_address_balances::Column::Balance)
                    .to_owned(),
            )
            .exec(db)
            .await?;
    }
    Ok(())
}

impl Properties {
    fn support_table_name() -> String {
        db_address_balances::Entity.table_name().to_string()
    }

    fn min_balance_for_holders() -> i64 {
        10_i64.pow(15)
    }

    fn step_duration_days() -> usize {
        1
    }

    fn max_rows_fetch_per_iteration() -> u64 {
        60_000
    }

    fn max_rows_insert_per_iteration() -> usize {
        20_000
    }
}

async fn get_unique_ordered_days<C>(
    blockscout: &C,
    maybe_from: Option<NaiveDate>,
    remote_fetch_timer: &mut AggregateTimer,
) -> Result<Vec<NaiveDate>, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    #[derive(Debug, FromQueryResult)]
    struct SelectResult {
        day: NaiveDate,
    }

    let mut query = blockscout_db::entity::address_coin_balances_daily::Entity::find()
        .select_only()
        .column(blockscout_db::entity::address_coin_balances_daily::Column::Day)
        .group_by(blockscout_db::entity::address_coin_balances_daily::Column::Day)
        .order_by_asc(blockscout_db::entity::address_coin_balances_daily::Column::Day);

    query = match maybe_from {
        Some(from) => {
            query.filter(blockscout_db::entity::address_coin_balances_daily::Column::Day.gte(from))
        }
        None => query,
    };
    let _timer = remote_fetch_timer.start_interval();
    let days = query
        .into_model::<SelectResult>()
        .all(blockscout)
        .await?
        .into_iter()
        .map(|result| result.day)
        .collect();

    Ok(days)
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NativeCoinHoldersGrowth =
    LocalDbChartSource<(), (), Create, Update, DefaultQueryVec<Properties>, Properties>;
pub type NativeCoinHoldersGrowthInt = MapParseTo<StripExt<NativeCoinHoldersGrowth>, i64>;
type NativeCoinHoldersGrowthS = StripExt<NativeCoinHoldersGrowth>;
pub type NativeCoinHoldersGrowthWeekly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinHoldersGrowthS, Week>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NativeCoinHoldersGrowthMonthly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinHoldersGrowthS, Month>,
    Batch36Months,
    MonthlyProperties,
>;
type NativeCoinHoldersGrowthMonthlyS = StripExt<NativeCoinHoldersGrowthMonthly>;
pub type NativeCoinHoldersGrowthYearly = DirectVecLocalDbChartSource<
    LastValueLowerResolution<NativeCoinHoldersGrowthMonthlyS, Year>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth() {
        simple_test_chart::<NativeCoinHoldersGrowth>(
            "update_native_coin_holders_growth",
            vec![
                ("2022-11-08", "0"),
                ("2022-11-09", "8"),
                ("2022-11-10", "8"),
                ("2022-11-11", "7"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth_weekly() {
        simple_test_chart::<NativeCoinHoldersGrowthWeekly>(
            "update_native_coin_holders_growth_weekly",
            vec![("2022-11-07", "7")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth_monthly() {
        simple_test_chart::<NativeCoinHoldersGrowthMonthly>(
            "update_native_coin_holders_growth_monthly",
            vec![("2022-11-01", "7")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coin_holders_growth_yearly() {
        simple_test_chart::<NativeCoinHoldersGrowthYearly>(
            "update_native_coin_holders_growth_yearly",
            vec![("2022-01-01", "7")],
        )
        .await;
    }
}
