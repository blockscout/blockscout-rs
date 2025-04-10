//! Tests for the case
//! - blockscout is fully indexed
//! - stats server is fully initialized with arbitrum charts disabled

use blockscout_service_launcher::{launcher::GracefulShutdownHandler, test_server::init_server};
use futures::FutureExt;
use stats::tests::{init_db::init_db, mock_blockscout::default_mock_blockscout_api};
use stats_server::stats;
use tokio::task::JoinSet;

use crate::{
    common::{
        get_test_stats_settings, run_consolidated_tests, wait_for_subset_to_update, ChartSubset,
    },
    it::mock_blockscout_simple::get_mock_blockscout,
};

use super::common_tests::{test_main_page_ok, test_transactions_page_ok};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_chart_pages_tests_with_disabled_chain_specific() {
    let test_name = "run_chart_pages_tests_with_disabled_chain_specific";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = default_mock_blockscout_api().await;
    let blockscout_indexed = true;
    let (mut settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    settings.enable_all_arbitrum = false;
    settings.enable_all_op_stack = false;
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_for_subset_to_update(&base, ChartSubset::AllCharts).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), false, blockscout_indexed).boxed(),
        test_transactions_page_ok(base, false).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
    stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}
