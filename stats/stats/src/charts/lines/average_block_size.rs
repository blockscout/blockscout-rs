use std::ops::Range;

use crate::{
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
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

use super::new_blocks::{NewBlocksInt, NewBlocksMonthlyInt};

pub struct AverageBlockSizeStatement;

impl StatementFromRange for AverageBlockSizeStatement {
    fn get_statement(range: Option<Range<DateTimeUtc>>, _: &BlockscoutMigrations) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    ROUND(AVG(blocks.size))::TEXT as value
                FROM blocks
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    consensus = true {filter}
                GROUP BY date
            "#,
            [],
            "blocks.timestamp",
            range,
        )
    }
}

pub type AverageBlockSizeRemote =
    RemoteDatabaseSource<PullAllWithAndSort<AverageBlockSizeStatement, NaiveDate, String>>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageBlockSize".into()
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

pub type AverageBlockSize =
    DirectVecLocalDbChartSource<AverageBlockSizeRemote, Batch30Days, Properties>;
type AverageBlockSizeS = StripExt<AverageBlockSize>;

pub type AverageBlockSizeWeekly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageBlockSizeS, f64>, NewBlocksInt, Week>>,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageBlockSizeMonthly = DirectVecLocalDbChartSource<
    MapToString<AverageLowerResolution<MapParseTo<AverageBlockSizeS, f64>, NewBlocksInt, Month>>,
    Batch36Months,
    MonthlyProperties,
>;
type AverageBlockSizeMonthlyS = StripExt<AverageBlockSizeMonthly>;
pub type AverageBlockSizeYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<
            MapParseTo<AverageBlockSizeMonthlyS, f64>,
            NewBlocksMonthlyInt,
            Year,
        >,
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
    async fn update_average_block_size() {
        simple_test_chart::<AverageBlockSize>(
            "update_average_block_size",
            vec![
                ("2022-11-09", "1000"),
                ("2022-11-10", "2726"),
                ("2022-11-11", "3247"),
                ("2022-11-12", "2904"),
                ("2022-12-01", "3767"),
                ("2023-01-01", "4630"),
                ("2023-02-01", "5493"),
                ("2023-03-01", "1356"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_size_weekly() {
        simple_test_chart::<AverageBlockSizeWeekly>(
            "update_average_block_size_weekly",
            vec![
                ("2022-11-07", "2785.5555555555557"),
                ("2022-11-28", "3767"),
                ("2022-12-26", "4630"),
                ("2023-01-30", "5493"),
                ("2023-02-27", "1356"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_size_monthly() {
        simple_test_chart::<AverageBlockSizeMonthly>(
            "update_average_block_size_monthly",
            vec![
                ("2022-11-01", "2785.5555555555557"),
                ("2022-12-01", "3767"),
                ("2023-01-01", "4630"),
                ("2023-02-01", "5493"),
                ("2023-03-01", "1356"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_size_yearly() {
        simple_test_chart::<AverageBlockSizeYearly>(
            "update_average_block_size_yearly",
            vec![
                ("2022-01-01", "2883.7"),
                ("2023-01-01", "3826.3333333333335"),
            ],
        )
        .await;
    }
}
