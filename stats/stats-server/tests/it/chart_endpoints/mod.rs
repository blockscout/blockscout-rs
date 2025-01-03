//! Test all chart endpoints.
//!
//! It is not done in separate tests in order
//! to simplify and optimize the server initialization
//! procedure (i.e. to not initialize the read-only
//! server each time).

use std::{path::PathBuf, str::FromStr};

use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server},
};
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
use stats_server::{stats, Settings};
use tokio::task::JoinSet;
use transactions_page::test_transactions_page_ok;

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
    let mut settings = Settings::build().expect("Failed to build settings");
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.charts_config = PathBuf::from_str("../config/charts.json").unwrap();
    settings.layout_config = PathBuf::from_str("../config/layout.json").unwrap();
    settings.update_groups_config = PathBuf::from_str("../config/update_groups.json").unwrap();
    settings.db_url = stats_db.db_url();
    settings.blockscout_db_url = blockscout_db.db_url();
    settings.blockscout_api_url = Some(url::Url::from_str(&blockscout_api.uri()).unwrap());

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let mut tests: JoinSet<_> = [
        test_lines_ok(base.clone()).boxed(),
        test_counters_ok(base.clone()).boxed(),
        test_main_page_ok(base.clone()).boxed(),
        test_transactions_page_ok(base.clone()).boxed(),
        test_contracts_page_ok(base).boxed(),
    ]
    .into_iter()
    .collect();
    let mut some_failed = false;
    while let Some(test_result) = tests.join_next().await {
        match test_result {
            Ok(()) => println!("test for chart endpoint ... ok"),
            Err(e) => {
                println!("test for chart endpoint ... fail\n{}", e);
                some_failed = true;
            }
        }
    }
    if some_failed {
        panic!("some tests failed")
    }
}
