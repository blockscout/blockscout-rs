use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server, send_get_request},
};
use chrono::NaiveDate;
use pretty_assertions::assert_eq;

use stats::tests::{
    init_db::init_db_all,
    mock_blockscout::{default_mock_blockscout_api, fill_mock_blockscout_data},
};
use stats_proto::blockscout::stats::v1::Counters;
use stats_server::{stats, Settings};

use std::{path::PathBuf, str::FromStr};

#[tokio::test]
#[ignore = "needs database"]
async fn test_counters_ok() {
    let (stats_db, blockscout_db) = init_db_all("test_counters_ok").await;
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

    let counters: Counters = send_get_request(&base, "/api/v1/counters").await;
    for counter in counters.counters.iter() {
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
    // Also check the order set in layout
    let counter_names: Vec<_> = counters.counters.iter().map(|c| c.id.as_str()).collect();
    let expected_counter_names: Vec<_> = [
        "averageBlockTime",
        "completedTxns",
        "lastNewContracts",
        "lastNewVerifiedContracts",
        "totalAccounts",
        "totalAddresses",
        "totalBlocks",
        "totalContracts",
        // "totalNativeCoinHolders",
        "totalNativeCoinTransfers",
        "totalTokens",
        "totalTxns",
        "totalVerifiedContracts",
        "yesterdayTxns",
        "newTxns24h",
        "pendingTxns",
        "txnsFee24h",
        "averageTxnFee24h",
    ]
    .into_iter()
    .collect();

    assert_eq!(counter_names, expected_counter_names);
}
