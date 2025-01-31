//! Test all chart endpoints.
//!
//! It is not done in separate tests in order
//! to simplify and optimize the server initialization
//! procedure (i.e. to not initialize the read-only
//! server each time).
//!
//! Note: all tests should be read only

use std::{str::FromStr, time::Instant};

use blockscout_service_launcher::{test_database::TestDbGuard, test_server::init_server};
use chrono::NaiveDate;
use contracts_page::test_contracts_page_ok;
use counters::test_counters_ok;
use futures::FutureExt;
use lines::test_lines_ok;
use main_page::test_main_page_ok;
use stats::tests::{
    init_db::{init_db, init_db_blockscout},
    mock_blockscout::{
        default_mock_blockscout_api, fill_mock_blockscout_data, mock_blockscout_api,
    },
};
use stats_server::stats;
use tokio::task::JoinSet;
use transactions_page::test_transactions_page_ok;
use wiremock::ResponseTemplate;

use crate::common::{get_test_stats_settings, run_consolidated_tests};

mod contracts_page;
mod counters;
mod lines;
mod main_page;
mod transactions_page;

#[tokio::test]
#[ignore = "needs database"]
async fn tests_with_mock_blockscout() {
    let test_name = "tests_with_mock_blockscout";
    let blockscout_db = init_db_blockscout(test_name).await;
    fill_mock_blockscout_data(&blockscout_db, NaiveDate::from_str("2023-03-01").unwrap()).await;

    let tests: JoinSet<_> = [
        test_chart_endpoints_ok(blockscout_db.clone()).boxed(),
        test_chart_endpoints_work_with_disabled_arbitrum(blockscout_db.clone()).boxed(),
        test_chart_endpoints_work_with_not_indexed_blockscout(blockscout_db).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

async fn test_chart_endpoints_ok(blockscout_db: TestDbGuard) {
    let test_name = "test_chart_endpoints_ok";
    let stats_db = init_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_lines_ok(base.clone()).boxed(),
        test_counters_ok(base.clone()).boxed(),
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

async fn test_chart_endpoints_work_with_not_indexed_blockscout(blockscout_db: TestDbGuard) {
    let test_name = "test_chart_endpoints_work_with_not_indexed_blockscout";
    let stats_db = init_db(test_name).await;
    let blockscout_api = mock_blockscout_api(ResponseTemplate::new(200).set_body_string(
        r#"{
            "finished_indexing": false,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "0.10",
            "indexed_internal_transactions_ratio": null
        }"#,
    ))
    .await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

async fn test_chart_endpoints_work_with_disabled_arbitrum(blockscout_db: TestDbGuard) {
    let test_name = "test_chart_endpoints_work_with_disabled_arbitrum";
    let stats_db = init_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);
    settings.enable_all_arbitrum = false;

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), false).boxed(),
        test_transactions_page_ok(base, false).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}
