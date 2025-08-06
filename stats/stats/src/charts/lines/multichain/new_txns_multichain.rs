use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryFullIndexerTimestampRange, data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                }, DirectVecLocalDbChartSource
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::IndexerMigrations,
    }, 
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartKey, ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewTxnsMultichainStatement;

impl StatementFromRange for NewTxnsMultichainStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    c.date,
                    SUM(c.daily_transactions_number)::TEXT AS value
                FROM counters_global_imported as c
                WHERE
                    c.daily_transactions_number IS NOT NULL
                    {filter}
                GROUP BY date
            "#,
            [],
            "c.date::timestamp",
            range
        )
    }
}

pub type NewTxnsMultichainRemote = RemoteDatabaseSource<
    PullAllWithAndSort<
        NewTxnsMultichainStatement,
        NaiveDate,
        String,
        QueryFullIndexerTimestampRange,
    >,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxnsMultichain".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

define_and_impl_resolution_properties!(
    define_and_impl: {
        WeeklyProperties: Week,
        MonthlyProperties: Month,
        YearlyProperties: Year,
    },
    base_impl: Properties
);

pub type NewTxnsMultichain =
    DirectVecLocalDbChartSource<NewTxnsMultichainRemote, Batch30Days, Properties>;
pub type NewTxnsMultichainInt = MapParseTo<StripExt<NewTxnsMultichain>, i64>;
pub type NewTxnsMultichainWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTxnsMultichainMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTxnsMultichainMonthlyInt =
    MapParseTo<StripExt<NewTxnsMultichainMonthly>, i64>;
pub type NewTxnsMultichainYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMultichainMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    // #[tokio::test]
    // #[ignore = "needs database to run"]
    // async fn update_native_coins_transfers() {
    //     simple_test_chart_with_migration_variants::<NewTxnsMultichain>(
    //         "update_new_txs_transfers",
    //         vec![
    //             ("2022-11-09", "2"),
    //             ("2022-11-10", "4"),
    //             ("2022-11-11", "4"),
    //             ("2022-11-12", "2"),
    //             ("2022-12-01", "2"),
    //             ("2023-02-01", "2"),
    //             ("2023-03-01", "1"),
    //         ],
    //     )
    //     .await;
    // }

    // #[tokio::test]
    // #[ignore = "needs database to run"]
    // async fn update_new_txs_weekly() {
    //     simple_test_chart_with_migration_variants::<NewTxnsMultichainWeekly>(
    //         "update_new_txs_weekly",
    //         vec![
    //             ("2022-11-07", "12"),
    //             ("2022-11-28", "2"),
    //             ("2023-01-30", "2"),
    //             ("2023-02-27", "1"),
    //         ],
    //     )
    //     .await;
    // }

    // #[tokio::test]
    // #[ignore = "needs database to run"]
    // async fn update_new_txs_monthly() {
    //     simple_test_chart_with_migration_variants::<NewTxnsMultichainMonthly>(
    //         "update_new_txs_monthly",
    //         vec![
    //             ("2022-11-01", "12"),
    //             ("2022-12-01", "2"),
    //             ("2023-02-01", "2"),
    //             ("2023-03-01", "1"),
    //         ],
    //     )
    //     .await;
    // }

    // #[tokio::test]
    // #[ignore = "needs database to run"]
    // async fn update_new_txs_yearly() {
    //     simple_test_chart_with_migration_variants::<NewTxnsMultichainYearly>(
    //         "update_new_txs_yearly",
    //         vec![("2022-01-01", "14"), ("2023-01-01", "3")],
    //     )
    //     .await;
    // }
}
