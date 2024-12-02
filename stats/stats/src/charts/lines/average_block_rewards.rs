use std::ops::Range;

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
    types::timespans::{Month, Week, Year},
    utils::sql_with_range_filter_opt,
    ChartProperties, Named,
};

use chrono::NaiveDate;
use chrono::{DateTime, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

use super::{NewBlockRewardsInt, NewBlockRewardsMonthlyInt};

const ETH: i64 = 1_000_000_000_000_000_000;

pub struct AverageBlockRewardsQuery;

impl StatementFromRange for AverageBlockRewardsQuery {
    fn get_statement(range: Option<Range<DateTime<Utc>>>, _: &BlockscoutMigrations) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    (AVG(block_rewards.reward) / $1)::FLOAT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true {filter}
                GROUP BY date
            "#,
            [ETH.into()],
            "blocks.timestamp",
            range,
        )
    }
}

pub type AverageBlockRewardsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<AverageBlockRewardsQuery, NaiveDate, f64, QueryAllBlockTimestampRange>,
>;

pub type AverageBlockRewardsRemoteString = MapToString<AverageBlockRewardsRemote>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "averageBlockRewards".into()
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

pub type AverageBlockRewards =
    DirectVecLocalDbChartSource<AverageBlockRewardsRemoteString, Batch30Days, Properties>;
type AverageBlockRewardsS = StripExt<AverageBlockRewards>;
pub type AverageBlockRewardsWeekly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageBlockRewardsS, f64>, NewBlockRewardsInt, Week>,
    >,
    Batch30Weeks,
    WeeklyProperties,
>;
pub type AverageBlockRewardsMonthly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<MapParseTo<AverageBlockRewardsS, f64>, NewBlockRewardsInt, Month>,
    >,
    Batch36Months,
    MonthlyProperties,
>;
type AverageBlockRewardsMonthlyS = StripExt<AverageBlockRewardsMonthly>;
pub type AverageBlockRewardsYearly = DirectVecLocalDbChartSource<
    MapToString<
        AverageLowerResolution<
            MapParseTo<AverageBlockRewardsMonthlyS, f64>,
            NewBlockRewardsMonthlyInt,
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
    async fn update_average_block_rewards() {
        simple_test_chart::<AverageBlockRewards>(
            "update_average_block_rewards",
            vec![
                ("2022-11-09", "3.3333333333333335"),
                ("2022-11-10", "2.5833333333333335"),
                ("2022-11-11", "2.111111111111111"),
                ("2022-11-12", "2.75"),
                ("2022-12-01", "2.5"),
                ("2023-01-01", "3.6666666666666665"),
                ("2023-02-01", "2.5"),
                ("2023-03-01", "3"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_rewards_weekly() {
        simple_test_chart::<AverageBlockRewardsWeekly>(
            "update_average_block_rewards_weekly",
            vec![
                ("2022-11-07", "2.5"),
                ("2022-11-28", "2.5"),
                ("2022-12-26", "3.6666666666666665"),
                ("2023-01-30", "2.5"),
                ("2023-02-27", "3"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_rewards_monthly() {
        simple_test_chart::<AverageBlockRewardsMonthly>(
            "update_average_block_rewards_monthly",
            vec![
                ("2022-11-01", "2.5"),
                ("2022-12-01", "2.5"),
                ("2023-01-01", "3.6666666666666665"),
                ("2023-02-01", "2.5"),
                ("2023-03-01", "3"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_rewards_yearly() {
        simple_test_chart::<AverageBlockRewardsYearly>(
            "update_average_block_rewards_yearly",
            vec![("2022-01-01", "2.5"), ("2023-01-01", "3.3")],
        )
        .await;
    }
}
