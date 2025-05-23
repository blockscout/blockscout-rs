use std::{collections::HashSet, ops::Range};

use crate::{
    charts::db_interaction::read::QueryAllBlockTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, MapToString, StripExt},
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
        types::BlockscoutMigrations,
    },
    define_and_impl_resolution_properties,
    lines::new_blocks::{NewBlocksInt, NewBlocksMonthlyInt},
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartKey, ChartProperties, Named,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct AverageGasLimitStatement;

impl StatementFromRange for AverageGasLimitStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &BlockscoutMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.gas_limit))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true {filter}
                GROUP BY date
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub type AverageGasLimitRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageGasLimitStatement, NaiveDate, String, QueryAllBlockTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageGasLimit".into()
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

pub type AverageGasLimit =
    DirectVecLocalDbChartSource<AverageGasLimitRemote, Batch30Days, Properties>;
type AverageGasLimitS = StripExt<AverageGasLimit>;
pub type AverageGasLimitFloat = MapParseTo<AverageGasLimitS, f64>;

pub type AverageGasLimitWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasLimitFloat, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageGasLimitWeeklyFloat = MapParseTo<StripExt<AverageGasLimitWeekly>, f64>;

pub type AverageGasLimitMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasLimitFloat, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
pub type AverageGasLimitMonthlyFloat = MapParseTo<StripExt<AverageGasLimitMonthly>, f64>;

pub type AverageGasLimitYearly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<AverageGasLimitMonthlyFloat, NewBlocksMonthlyInt, Year>>,
    Batch30Years,
    YearlyProperties,
>;
pub type AverageGasLimitYearlyFloat = MapParseTo<StripExt<AverageGasLimitYearly>, f64>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit() {
        simple_test_chart::<AverageGasLimit>(
            "update_average_gas_limit",
            vec![
                ("2022-11-09", "12500000"),
                ("2022-11-10", "12500000"),
                ("2022-11-11", "30000000"),
                ("2022-11-12", "30000000"),
                ("2022-12-01", "30000000"),
                ("2023-01-01", "30000000"),
                ("2023-02-01", "30000000"),
                ("2023-03-01", "30000000"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit_weekly() {
        simple_test_chart::<AverageGasLimitWeekly>(
            "update_average_gas_limit_weekly",
            vec![
                ("2022-11-07", "22222222.222222224"),
                ("2022-11-28", "30000000"),
                ("2022-12-26", "30000000"),
                ("2023-01-30", "30000000"),
                ("2023-02-27", "30000000"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit_monthly() {
        simple_test_chart::<AverageGasLimitMonthly>(
            "update_average_gas_limit_monthly",
            vec![
                ("2022-11-01", "22222222.222222224"),
                ("2022-12-01", "30000000"),
                ("2023-01-01", "30000000"),
                ("2023-02-01", "30000000"),
                ("2023-03-01", "30000000"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_gas_limit_yearly() {
        simple_test_chart::<AverageGasLimitYearly>(
            "update_average_gas_limit_yearly",
            vec![("2022-01-01", "23000000"), ("2023-01-01", "30000000")],
        )
        .await;
    }
}
