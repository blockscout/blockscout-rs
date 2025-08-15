//! Number of block rewards in each timespan.
//!
//! Intended for correct calculation of `average_block_rewards` rather
//! than being an actual chart, since doesn't seem to be a useful info.

use std::{collections::HashSet, ops::Range};

use crate::{
    ChartKey, ChartProperties, Named,
    charts::db_interaction::read::QueryFullIndexerTimestampRange,
    data_source::{
        kinds::{
            data_manipulation::{
                map::{MapParseTo, StripExt},
                resolutions::sum::SumLowerResolution,
            },
            local_db::{
                DirectVecLocalDbChartSource, parameters::update::batching::parameters::Batch30Days,
            },
            remote_db::{PullAllWithAndSort, RemoteDatabaseSource, StatementFromRange},
        },
        types::IndexerMigrations,
    },
    types::timespans::Month,
    utils::sql_with_range_filter_opt,
};

use chrono::{DateTime, NaiveDate, Utc};
use entity::sea_orm_active_enums::ChartType;
use sea_orm::{DbBackend, Statement};

pub struct NewBlockRewardsStatement;

impl StatementFromRange for NewBlockRewardsStatement {
    fn get_statement(
        range: Option<Range<DateTime<Utc>>>,
        _: &IndexerMigrations,
        _: &HashSet<ChartKey>,
    ) -> Statement {
        sql_with_range_filter_opt!(
            DbBackend::Postgres,
            r#"
                SELECT
                    DATE(blocks.timestamp) as date,
                    COUNT(*)::TEXT as value
                FROM block_rewards
                JOIN blocks ON block_rewards.block_hash = blocks.hash
                WHERE
                    blocks.timestamp != to_timestamp(0) AND
                    blocks.consensus = true {filter}
                GROUP BY date;
            "#,
            [],
            "blocks.timestamp",
            range
        )
    }
}

pub type NewBlockRewardsRemote = RemoteDatabaseSource<
    PullAllWithAndSort<NewBlockRewardsStatement, NaiveDate, String, QueryFullIndexerTimestampRange>,
>;

pub struct Properties;

impl Named for Properties {
    fn name() -> String {
        "newBlockRewards".into()
    }
}

impl ChartProperties for Properties {
    type Resolution = NaiveDate;

    fn chart_type() -> ChartType {
        ChartType::Line
    }
}

pub type NewBlockRewards =
    DirectVecLocalDbChartSource<NewBlockRewardsRemote, Batch30Days, Properties>;
pub type NewBlockRewardsInt = MapParseTo<StripExt<NewBlockRewards>, i64>;
pub type NewBlockRewardsMonthlyInt = SumLowerResolution<NewBlockRewardsInt, Month>;

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use blockscout_metrics_tools::AggregateTimer;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::{
        data_source::{DataSource, UpdateContext, UpdateParameters, types::IndexerMigrations},
        range::UniversalRange,
        tests::{
            init_db::init_db_all,
            mock_blockscout::fill_mock_blockscout_data,
            simple_test::{map_str_tuple_to_owned, simple_test_chart},
        },
        types::Timespan,
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_block_rewards() {
        simple_test_chart::<NewBlockRewards>(
            "update_new_block_rewards",
            vec![
                ("2022-11-09", "3"),
                ("2022-11-10", "6"),
                ("2022-11-11", "9"),
                ("2022-11-12", "2"),
                ("2022-12-01", "3"),
                ("2023-01-01", "3"),
                ("2023-02-01", "1"),
                ("2023-03-01", "1"),
            ],
        )
        .await;
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn update_new_block_rewards_monthly() {
        let _ = tracing_subscriber::fmt::try_init();
        let expected = map_str_tuple_to_owned(vec![
            ("2022-11-01", "20"),
            ("2022-12-01", "3"),
            ("2023-01-01", "3"),
            ("2023-02-01", "1"),
            ("2023-03-01", "1"),
        ]);
        let (db, blockscout) = init_db_all("update_new_block_rewards_monthly_int").await;
        let current_time = chrono::DateTime::from_str("2023-03-01T12:00:00Z").unwrap();
        let current_date = current_time.date_naive();
        NewBlockRewardsMonthlyInt::init_recursively(&db, &current_time)
            .await
            .unwrap();
        fill_mock_blockscout_data(&blockscout, current_date).await;

        let parameters = UpdateParameters {
            stats_db: &db,
            is_multichain_mode: false,
            indexer_db: &blockscout,
            indexer_applied_migrations: IndexerMigrations::latest(),
            enabled_update_charts_recursive: NewBlockRewardsMonthlyInt::all_dependencies_chart_keys(
            ),
            update_time_override: Some(current_time),
            force_full: false,
        };
        let cx = UpdateContext::from_params_now_or_override(parameters.clone());
        NewBlockRewardsMonthlyInt::update_recursively(&cx)
            .await
            .unwrap();
        let mut timer = AggregateTimer::new();
        let data: Vec<_> =
            NewBlockRewardsMonthlyInt::query_data(&cx, UniversalRange::full(), &mut timer)
                .await
                .unwrap()
                .into_iter()
                .map(|p| (p.timespan.into_date().to_string(), p.value.to_string()))
                .collect();
        assert_eq!(data, expected);
    }
}
