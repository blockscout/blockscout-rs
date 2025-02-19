//! Tests for the case
//! - blockscout is fully indexed
//! - stats server is fully initialized with arbitrum charts disabled

use std::{str::FromStr, time::Duration};

use blockscout_service_launcher::test_server::init_server;
use chrono::NaiveDate;
use futures::FutureExt;
use stats::tests::{
    init_db::{init_db, init_db_blockscout},
    mock_blockscout::{default_mock_blockscout_api, fill_mock_blockscout_data},
};
use stats_server::stats;
use tokio::task::JoinSet;

use crate::common::{
    get_test_stats_settings, healthcheck_successful, run_consolidated_tests,
    wait_for_subset_to_update, ChartSubset,
};

use super::common_tests::{test_main_page_ok, test_transactions_page_ok};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_chart_pages_tests_with_disabled_arbitrum() {
    let test_name = "run_chart_pages_tests_with_disabled_arbitrum";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    settings.enable_all_arbitrum = false;

    init_server(
        || stats(settings),
        &base,
        Some(Duration::from_secs(60)),
        Some(healthcheck_successful),
    )
    .await;

    wait_for_subset_to_update(&base, ChartSubset::AllCharts).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), false).boxed(),
        test_transactions_page_ok(base, false).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}
