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
use crate::common::{
    get_test_stats_settings, healthcheck_successful, run_consolidated_tests,
    wait_for_subset_to_update, ChartSubset,
};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_fully_initialized_stats_tests() {
    let test_name = "run_fully_initialized_stats_tests";
    let stats_db = init_db(test_name).await;
    // let blockscout_db = get_mock_blockscout().await;
    let blockscout_db = stats::tests::init_db::init_db_blockscout(test_name).await;
    stats::tests::mock_blockscout::fill_mock_blockscout_data(
        &blockscout_db,
        <chrono::NaiveDate as std::str::FromStr>::from_str("2023-03-01").unwrap(),
    )
    .await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(
        move || stats(settings),
        &base,
        Some(Duration::from_secs(60)),
        Some(healthcheck_successful),
    )
    .await;
    sleep(Duration::from_secs(10)).await;
    wait_for_subset_to_update(&base, ChartSubset::AllCharts).await;

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
