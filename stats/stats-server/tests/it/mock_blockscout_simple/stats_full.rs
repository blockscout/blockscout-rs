//! Tests for the case
//! - blockscout is fully indexed
//! - stats server is fully enabled & initialized

use blockscout_service_launcher::{launcher::GracefulShutdownHandler, test_server::init_server};
use futures::FutureExt;
use stats::tests::{init_db::init_db, mock_blockscout::default_mock_blockscout_api};
use stats_server::stats;
use tokio::task::JoinSet;

use super::common_tests::{
    test_contracts_page_ok, test_counters_ok, test_lines_ok, test_main_page_ok,
    test_transactions_page_ok,
};
use crate::{
    common::{
        ChartSubset, get_test_stats_settings, run_consolidated_tests, wait_for_subset_to_update,
    },
    it::mock_blockscout_simple::{get_mock_blockscout, get_mock_zetachain_cctx},
};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_fully_initialized_stats_tests() {
    let test_name = "run_fully_initialized_stats_tests";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = default_mock_blockscout_api().await;
    let zetachain_cctx_db = get_mock_zetachain_cctx().await;
    let (blockscout_indexed, user_ops_indexed, zetachain_indexed) = (true, true, true);
    let (settings, base) = get_test_stats_settings(
        &stats_db,
        blockscout_db,
        &blockscout_api,
        Some(zetachain_cctx_db),
    );
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_for_subset_to_update(&base, ChartSubset::AllCharts).await;

    let tests: JoinSet<_> = [
        test_lines_ok(
            base.clone(),
            blockscout_indexed,
            user_ops_indexed,
            zetachain_indexed,
        )
        .boxed(),
        test_counters_ok(
            base.clone(),
            blockscout_indexed,
            user_ops_indexed,
            zetachain_indexed,
        )
        .boxed(),
        test_main_page_ok(base.clone(), true, blockscout_indexed).boxed(),
        test_transactions_page_ok(base.clone(), true, zetachain_indexed).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
    stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}
