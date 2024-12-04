use std::ops::Range;

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
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

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewTxnsStatement;

impl StatementFromRange for NewTxnsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        completed_migrations: &BlockscoutMigrations,
    ) -> Statement {
        if completed_migrations.denormalization {
            sql_with_range_filter_opt!(
                DbBackend::Postgres,
                r#"
                    SELECT
                        date(t.block_timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    WHERE
                        t.block_timestamp != to_timestamp(0) AND
                        t.block_consensus = true {filter}
                    GROUP BY date;
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
                        date(b.timestamp) as date,
                        COUNT(*)::TEXT as value
                    FROM transactions t
                    JOIN blocks       b ON t.block_hash = b.hash
                    WHERE
                        b.timestamp != to_timestamp(0) AND
                        b.consensus = true {filter}
                    GROUP BY date;
                "#,
                [],
                "b.timestamp",
                range
            )
        }
    }
}

pub type NewTxnsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<NewTxnsStatement, NaiveDate, String, QueryAllBlockTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newTxns".into()
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

pub type NewTxns = DirectVecLocalDbChartSource<NewTxnsRemote, Batch30Days, Properties>;
pub type NewTxnsInt = MapParseTo<StripExt<NewTxns>, i64>;
pub type NewTxnsWeekly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type NewTxnsMonthly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type NewTxnsMonthlyInt = MapParseTo<StripExt<NewTxnsMonthly>, i64>;
pub type NewTxnsYearly = DirectVecLocalDbChartSource<
    MapToString<SumLowerResolution<NewTxnsMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::{
        ranged_test_chart_with_migration_variants, simple_test_chart_with_migration_variants,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns() {
        simple_test_chart_with_migration_variants::<NewTxns>(
            "update_new_txns",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_weekly() {
        simple_test_chart_with_migration_variants::<NewTxnsWeekly>(
            "update_new_txns_weekly",
            vec![
                ("2022-11-07", "36"),
                ("2022-11-28", "5"),
                ("2022-12-26", "1"),
                ("2023-01-30", "4"),
                ("2023-02-27", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_monthly() {
        simple_test_chart_with_migration_variants::<NewTxnsMonthly>(
            "update_new_txns_monthly",
            vec![
                ("2022-11-01", "36"),
                ("2022-12-01", "5"),
                ("2023-01-01", "1"),
                ("2023-02-01", "4"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_txns_yearly() {
        simple_test_chart_with_migration_variants::<NewTxnsYearly>(
            "update_new_txns_yearly",
            vec![("2022-01-01", "41"), ("2023-01-01", "6")],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn ranged_update_new_txns() {
        ranged_test_chart_with_migration_variants::<NewTxns>(
            "ranged_update_new_txns",
            vec![
                ("2022-11-09", "5"),
                ("2022-11-10", "12"),
                ("2022-11-11", "14"),
                ("2022-11-12", "5"),
                ("2022-12-01", "5"),
            ],
            "2022-11-08".parse().unwrap(),
            "2022-12-01".parse().unwrap(),
            None,
        )
        .await;
    }
}
