//! Tests for the case
//! - blockscout is fully indexed
//! - stats server is fully initialized with arbitrum charts disabled

use std::time::Duration;

use blockscout_service_launcher::{test_database::TestDbGuard, test_server::init_server};
use futures::FutureExt;
use stats::tests::{init_db::init_db, mock_blockscout::default_mock_blockscout_api};
use stats_server::stats;
use tokio::{task::JoinSet, time::sleep};

use crate::common::{get_test_stats_settings, run_consolidated_tests};

use super::common_tests::{test_main_page_ok, test_transactions_page_ok};

pub async fn run_chart_pages_tests_with_disabled_arbitrum(blockscout_db: TestDbGuard) {
    let test_name = "run_chart_pages_tests_with_disabled_arbitrum";
    let stats_db = init_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);
    settings.enable_all_arbitrum = false;

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    sleep(Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), false).boxed(),
        test_transactions_page_ok(base, false).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}
