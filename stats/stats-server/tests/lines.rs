use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use stats::tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use stats_server::{stats, Settings};
use std::{path::PathBuf, str::FromStr};

fn client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_total_retry_duration(std::time::Duration::from_secs(10));
    ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

#[tokio::test]
#[ignore = "needs database"]
async fn test_lines_ok() {
    let db_url = std::env::var("DATABASE_URL").expect("no DATABASE_URL env");
    let (_stats, blockscout) = init_db_all("test_lines_ok", Some(db_url.clone())).await;
    let stats_db_url = format!("{db_url}/test_lines_ok",);
    let blockscout_db_url = format!("{db_url}/test_lines_ok_blockscout",);
    fill_mock_blockscout_data(&blockscout, "2022-11-11").await;

    let mut settings = Settings::default();
    settings.charts_config = PathBuf::from_str("../config/charts.toml").unwrap();
    settings.server.grpc.enabled = false;
    settings.metrics.enabled = false;
    settings.jaeger.enabled = false;
    settings.db_url = stats_db_url;
    settings.blockscout_db_url = blockscout_db_url;

    let base = format!("http://{}", settings.server.http.addr);

    let _server_handle = {
        let settings = settings.clone();
        tokio::spawn(async move { stats(settings).await.unwrap() })
    };
    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    let client = client();

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
        "nativeCoinHoldersGrowth",
        "nativeCoinSupply",
        "newNativeCoinHolders",
        "newBlocks",
        "newNativeCoinTransfers",
        "newTxns",
        "txnsFee",
        "txnsGrowth",
        "txnsSuccessRate",
    ] {
        let resp = client
            .get(format!("{base}/api/v1/lines/{line_name}"))
            .send()
            .await
            .expect("failed to connect to server");
        let s = resp.status();
        assert_eq!(s, 200, "invalid status for chart '{line_name}': {s:?}");

        let chart: serde_json::Value = resp
            .json()
            .await
            .expect("failed to convert response to json");
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
