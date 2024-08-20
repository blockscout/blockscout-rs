use std::ops::Range;

use crate::{
    data_source::kinds::{
        data_manipulation::{
            map::{MapParseTo, MapToString},
            resolutions::average::AverageLowerResolution,
        },
        local_db::{
            parameters::update::batching::parameters::{
                Batch30Days, Batch30Weeks, Batch30Years, Batch36Months,
            },
            DirectVecLocalDbChartSource,
        },
        remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
    },
    define_and_impl_resolution_properties,
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

use super::new_blocks::{NewBlocksInt, NewBlocksMonthlyInt};

pub struct TxnsSuccessRateStatement;

impl StatementFromRange for TxnsSuccessRateStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(b.timestamp) as date,
                    COUNT(CASE WHEN t.error IS NULL THEN 1 END)::FLOAT
                        / COUNT(*)::FLOAT as value
                FROM transactions t
                JOIN blocks       b ON t.block_hash = b.hash
                WHERE
                    b.timestamp != to_timestamp(0) AND
                    b.consensus = true AND
                    t.block_hash IS NOT NULL AND
                    (t.error IS NULL OR t.error::text != 'dropped/replaced') {filter}
                GROUP BY DATE(b.timestamp)
                "#,
            [],
            "b.timestamp",
            range
        )
    }
}

pub type TxnsSuccessRateRemote =
    RemoteDatabaseSource<PullAllWithAndSort<TxnsSuccessRateStatement, NaiveDate, f64>>;

pub type TxnsSuccessRateRemoteString = MapToString<TxnsSuccessRateRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "txnsSuccessRate".into()
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

pub type TxnsSuccessRate =
    DirectVecLocalDbChartSource<TxnsSuccessRateRemoteString, Batch30Days, Properties>;
pub type TxnsSuccessRateWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<TxnsSuccessRate, f64>, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type TxnsSuccessRateMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<TxnsSuccessRate, f64>, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type TxnsSuccessRateYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<TxnsSuccessRateMonthly, f64>, NewBlocksMonthlyInt, Year>,
    >,
    Batch30Years,
    YearlyProperties,
>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate() {
        simple_test_chart::<TxnsSuccessRate>(
            "update_txns_success_rate",
            vec![
                ("2022-11-09", "1"),
                ("2022-11-10", "1"),
                ("2022-11-11", "1"),
                ("2022-11-12", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate_weekly() {
        simple_test_chart::<TxnsSuccessRateWeekly>(
            "update_txns_success_rate_weekly",
            vec![
                ("2022-11-07", "1"),
                ("2022-11-28", "1"),
                ("2022-12-26", "1"),
                ("2023-01-30", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate_monthly() {
        simple_test_chart::<TxnsSuccessRateMonthly>(
            "update_txns_success_rate_monthly",
            vec![
                ("2022-11-01", "1"),
                ("2022-12-01", "1"),
                ("2023-01-01", "1"),
                ("2023-02-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_txns_success_rate_yearly() {
        simple_test_chart::<TxnsSuccessRateYearly>(
            "update_txns_success_rate_yearly",
            vec![("2022-01-01", "1"), ("2023-01-01", "1")],
        )
        .await;
    }
}
