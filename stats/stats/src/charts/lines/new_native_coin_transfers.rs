use std::ops::Range;

use crate::{
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                parameters::update::batching::parameters::{
                    Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
                },
                DirectVecLocalDbChartSource,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

pub struct NewNativeCoinTransfersStatement;

impl StatementFromRange for NewNativeCoinTransfersStatement {
    fn get_statement(
        range: Option<Range<DateTimeUtc>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(t.block_timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    WHERE
                        t.block_timestamp != to_timestamp(0) AND
                        t.block_consensus = true AND
                        LENGTH(t.input) = 0 AND
                        t.value >= 0 {filter}
                    GROUP BY date
                "#,
                [],
                "t.block_timestamp",
                range
            )
        } else {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        DATE(b.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true AND
                        LENGTH(t.input) = 0 AND
                        t.value >= 0 {filter}
                    GROUP BY date
                "#,
                [],
                "b.timestamp",
                range
            )
        }
    }
}

pub type NewNativeCoinTransfersRemote =
    RemoteDatabaseSource<PullAllWithAndSort<NewNativeCoinTransfersStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newNativeCoinTransfers".into()
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

pub type NewNativeCoinTransfers =
    DirectVecLocalDbChartSource<NewNativeCoinTransfersRemote, Batch30Days, Properties>;
pub type NewNativeCoinTransfersInt = MapParseTo<NewNativeCoinTransfers, i64>;
pub type NewNativeCoinTransfersWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinTransfersInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewNativeCoinTransfersMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinTransfersInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewNativeCoinTransfersMonthlyInt = MapParseTo<NewNativeCoinTransfersMonthly, i64>;
pub type NewNativeCoinTransfersYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewNativeCoinTransfersMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart_with_migration_variants;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers() {
        simple_test_chart_with_migration_variants::<NewNativeCoinTransfers>(
            "update_native_coins_transfers",
            vec![
                ("2022-11-09", "2"),
                ("2022-11-10", "4"),
                ("2022-11-11", "4"),
                ("2022-11-12", "2"),
                ("2022-12-01", "2"),
                ("2023-02-01", "2"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers_weekly() {
        simple_test_chart_with_migration_variants::<NewNativeCoinTransfersWeekly>(
            "update_native_coins_transfers_weekly",
            vec![
                ("2022-11-07", "12"),
                ("2022-11-28", "2"),
                ("2023-01-30", "2"),
                ("2023-02-27", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers_monthly() {
        simple_test_chart_with_migration_variants::<NewNativeCoinTransfersMonthly>(
            "update_native_coins_transfers_monthly",
            vec![
                ("2022-11-01", "12"),
                ("2022-12-01", "2"),
                ("2023-02-01", "2"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_native_coins_transfers_yearly() {
        simple_test_chart_with_migration_variants::<NewNativeCoinTransfersYearly>(
            "update_native_coins_transfers_yearly",
            vec![("2022-01-01", "14"), ("2023-01-01", "3")],
        )
        .await;
    }
}
