use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server::{get_test_server_settings, init_server, send_get_request},
};
use stats::tests::mock_blockscout::fill_mock_blockscout_data;
use stats_server::{stats, Settings};
use std::{path::PathBuf, str::FromStr};

#[tokio::test]
#[ignore = "needs database"]
async fn test_lines_ok() {
    let stats_db = TestDbGuard::new::<stats_migration::Migrator>("test_lines_ok").await;
    let blockscout_db =
        TestDbGuard::new::<blockscout_db::migration::Migrator>("test_lines_ok_blockscout").await;
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

    let line_charts: stats_proto::blockscout::stats::v1::LineCharts =
        send_get_request(&base, "/api/v1/lines").await;
    let sections: Vec<&str> = line_charts
        .sections
        .iter()
        .map(|sec| sec.id.as_str())
        .collect();
    let expected_sections = [
        "accounts",
        "transactions",
        "blocks",
        "tokens",
        "gas",
        "contracts",
    ];
    assert_eq!(sections, expected_sections, "wrong sections response");

    for line_name in [
        "accountsGrowth",
        "activeAccounts",
        "averageBlockSize",
        "averageBlockRewards",
        "newAccounts",
        "averageGasLimit",
        "averageGasPrice",
        "averageTxnFee",
        "gasUsedGrowth",
        // "nativeCoinHoldersGrowth",
        // "nativeCoinSupply",
        // "newNativeCoinHolders",
        "newBlocks",
        "newNativeCoinTransfers",
        "newTxns",
        "txnsFee",
        "txnsGrowth",
        "txnsSuccessRate",
        "newVerifiedContracts",
        "newContracts",
        "verifiedContractsGrowth",
        "contractsGrowth",
    ] {
        let chart: serde_json::Value =
            send_get_request(&base, &format!("/api/v1/lines/{line_name}")).await;
        let chart = chart
            .as_object()
            .expect("response has to be json object")
            .get("chart")
            .expect("response doesn't have 'chart' field")
            .as_array()
            .expect("'chart' field has to be json array");

        assert!(!chart.is_empty(), "chart '{line_name}' is empty");
    }
}
