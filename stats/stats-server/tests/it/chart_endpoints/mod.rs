//! Test all chart endpoints.
//!
//! It is not done in separate tests in order
//! to simplify and optimize the server initialization
//! procedure (i.e. to not initialize the read-only
//! server each time).

use std::str::FromStr;

use blockscout_service_launcher::test_server::init_server;
use chrono::NaiveDate;
use contracts_page::test_contracts_page_ok;
use counters::test_counters_ok;
use futures::FutureExt;
use lines::test_lines_ok;
use main_page::test_main_page_ok;
use stats::tests::{
    init_db::init_db_all,
    mock_blockscout::{default_mock_blockscout_api, fill_mock_blockscout_data},
};
use stats_server::stats;
use tokio::task::JoinSet;
use transactions_page::test_transactions_page_ok;

use crate::common::get_test_stats_settings;

mod contracts_page;
mod counters;
mod lines;
mod main_page;
mod transactions_page;

#[tokio::test]
#[ignore = "needs database"]
async fn test_chart_endpoints_ok() {
    let (stats_db, blockscout_db) = init_db_all("test_chart_endpoints_ok").await;
    let blockscout_api = default_mock_blockscout_api().await;
    fill_mock_blockscout_data(&blockscout_db, NaiveDate::from_str("2023-03-01").unwrap()).await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(7)).await;

    let mut tests: JoinSet<_> = [
        test_lines_ok(base.clone()).boxed(),
        test_counters_ok(base.clone()).boxed(),
        test_main_page_ok(base.clone()).boxed(),
        test_transactions_page_ok(base.clone()).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    let mut failed = 0;
    let total = tests.len();
    println!("running {total} endpoint tests");
    while let Some(test_result) = tests.join_next().await {
        let result_string_start = "(endpoint tests) stats endpoint test ... ";
        match test_result {
            Ok(()) => println!("{result_string_start}ok"),
            Err(e) => {
                println!("{result_string_start}fail\nerror: {e}",);
                failed += 1;
            }
        }
    }
    let passed = total - failed;
    let msg = format!("{passed}/{total} endpoint tests passed");
    if failed > 0 {
        panic!("{}", msg)
    } else {
        println!("{}", msg)
    }
}
