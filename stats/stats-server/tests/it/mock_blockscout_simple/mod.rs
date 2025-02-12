//! Tests for fully initialized blockscout without reindexing

use std::str::FromStr;

use chrono::NaiveDate;
use futures::FutureExt;
use stats::tests::{init_db::init_db_blockscout, mock_blockscout::fill_mock_blockscout_data};
use tokio::task::JoinSet;

use crate::common::run_consolidated_tests;

mod common_tests;
mod stats_full;
mod stats_no_arbitrum;
mod stats_not_indexed;
mod stats_not_updated;

/// Should be increased with the number of initialized stats instances,
/// because they will take more time to load concurrently
const STATS_INIT_WAIT_S: u64 = 15;

/// Tests that do not change the state of blockscout db
#[tokio::test]
#[ignore = "needs database"]
async fn tests_with_mock_blockscout() {
    let test_name = "tests_with_mock_blockscout";
    let blockscout_db = init_db_blockscout(test_name).await;
    fill_mock_blockscout_data(&blockscout_db, NaiveDate::from_str("2023-03-01").unwrap()).await;

    let tests: JoinSet<_> = [
        stats_full::run_fully_initialized_stats_tests(blockscout_db.clone()).boxed(),
        stats_no_arbitrum::run_chart_pages_tests_with_disabled_arbitrum(blockscout_db.clone())
            .boxed(),
        stats_not_indexed::run_tests_with_nothing_indexed(blockscout_db.clone()).boxed(),
        stats_not_indexed::run_tests_with_user_ops_not_indexed(blockscout_db.clone()).boxed(),
        stats_not_updated::run_tests_with_charts_not_updated(blockscout_db).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}
