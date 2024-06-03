use crate::{
    charts::db_interaction::types::DateValueDouble,
    data_source::kinds::{
        adapter::{ToStringAdapter, ToStringAdapterWrapper},
        remote::{RemoteSource, RemoteSourceWrapper},
        updateable_chart::batch::clone::{CloneChart, CloneChartWrapper},
    },
    utils::sql_with_range_filter_opt,
    Chart, Named,
};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{prelude::*, DbBackend, Statement};

const ETH: i64 = 1_000_000_000_000_000_000;

pub struct AverageBlockRewardsRemote;

impl RemoteSource for AverageBlockRewardsRemote {
    type Point = DateValueDouble;

    fn get_query(range: Option<std::ops::RangeInclusive<DateTimeUtc>>) -> Statement {
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

pub struct AverageBlockRewardsRemoteString;

impl ToStringAdapter for AverageBlockRewardsRemoteString {
    type InnerSource = RemoteSourceWrapper<AverageBlockRewardsRemote>;
    type ConvertFrom = <AverageBlockRewardsRemote as RemoteSource>::Point;
}

pub struct AverageBlockRewardsInner;

impl Named for AverageBlockRewardsInner {
    const NAME: &'static str = "averageBlockRewards";
}

impl Chart for AverageBlockRewardsInner {
    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

impl CloneChart for AverageBlockRewardsInner {
    type Dependency = ToStringAdapterWrapper<AverageBlockRewardsRemoteString>;
}

pub type AverageBlockRewards = CloneChartWrapper<AverageBlockRewardsInner>;

#[cfg(test)]
mod tests {
    use super::AverageBlockRewards;
    use crate::tests::simple_test::simple_test_chart;

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_average_block_rewards() {
        simple_test_chart::<AverageBlockRewards>(
            "update_average_block_rewards",
            vec![
                ("2022-11-09", "0"),
                ("2022-11-10", "2"),
                ("2022-11-11", "1.75"),
                ("2022-11-12", "3"),
                ("2022-12-01", "4"),
                ("2023-01-01", "0"),
                ("2023-02-01", "1"),
                ("2023-03-01", "2"),
            ],
        )
        .await;
    }
}
