use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server::{get_test_server_settings, init_server, send_get_request},
};
use stats::tests::mock_blockscout::fill_mock_blockscout_data;
use stats_proto::blockscout::stats::v1::Counters;
use stats_server::{stats, Settings};
use std::{collections::HashSet, path::PathBuf, str::FromStr};

#[tokio::test]
#[ignore = "needs database"]
async fn test_counters_ok() {
    let stats_db = TestDbGuard::new::<stats_migration::Migrator>("test_counters_ok").await;
    let blockscout_db =
        TestDbGuard::new::<blockscout_db::migration::Migrator>("test_counters_ok_blockscout").await;
    fill_mock_blockscout_data(&blockscout_db.client(), "2023-03-01").await;

    let (server_settings, base) = get_test_server_settings();
    let mut settings = Settings::default();
    settings.server = server_settings;
    settings.charts_config = PathBuf::from_str("../config/charts.json").unwrap();
    settings.metrics.enabled = false;
    settings.jaeger.enabled = false;
    settings.db_url = stats_db.db_url().to_string();
    settings.blockscout_db_url = blockscout_db.db_url().to_string();

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let counters: Counters = send_get_request(&base, "/api/v1/counters").await;
    for counter in counters.counters.iter() {
        assert!(!counter.description.is_empty());
        assert!(!counter.title.is_empty());
    }
    let counter_names: HashSet<_> = counters.counters.iter().map(|c| c.id.as_str()).collect();
    let expected_counter_names: HashSet<_> = [
        "totalBlocks",
        "totalAddresses",
        "averageBlockTime",
        "completedTxns",
        "totalTxns",
        "totalAccounts",
        "totalTokens",
        // "totalNativeCoinHolders",
        "totalNativeCoinTransfers",
        "lastNewContracts",
        "lastNewVerifiedContracts",
        "totalContracts",
        "totalVerifiedContracts",
    ]
    .into_iter()
    .collect();

    assert_eq!(counter_names, expected_counter_names);
}
