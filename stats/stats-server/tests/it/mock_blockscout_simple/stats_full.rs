//! Tests for the case
//! - blockscout is fully indexed
//! - stats server is fully enabled & initialized

use std::time::Duration;

use blockscout_service_launcher::test_server::init_server;
use futures::FutureExt;
use stats::tests::{init_db::init_db, mock_blockscout::default_mock_blockscout_api};
use stats_server::stats;
use tokio::{task::JoinSet, time::sleep};

use super::common_tests::{
    test_contracts_page_ok, test_counters_ok, test_lines_ok, test_main_page_ok,
    test_transactions_page_ok,
};
use crate::{
    common::{get_test_stats_settings, run_consolidated_tests},
    it::mock_blockscout_simple::{get_mock_blockscout, STATS_INIT_WAIT_S},
};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_fully_initialized_stats_tests() {
    let test_name = "run_fully_initialized_stats_tests";
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    sleep(Duration::from_secs(STATS_INIT_WAIT_S)).await;

    let tests: JoinSet<_> = [
        test_lines_ok(base.clone(), true, true).boxed(),
        test_counters_ok(base.clone(), true, true).boxed(),
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}
