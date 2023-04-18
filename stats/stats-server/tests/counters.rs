use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use stats::tests::{init_db::init_db_all, mock_blockscout::fill_mock_blockscout_data};
use stats_proto::blockscout::stats::v1::Counters;
use stats_server::{stats, Settings};
use std::{collections::HashSet, path::PathBuf, str::FromStr};

fn client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder()
        .build_with_total_retry_duration(std::time::Duration::from_secs(10));
    ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build()
}

#[tokio::test]
#[ignore = "needs database"]
async fn test_counters_ok() {
    let db_url = std::env::var("DATABASE_URL").expect("no DATABASE_URL env");
    let (_stats, blockscout) = init_db_all("test_counters_ok", Some(db_url.clone())).await;
    let stats_db_url = format!("{db_url}/test_counters_ok",);
    let blockscout_db_url = format!("{db_url}/test_counters_ok_blockscout",);
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

    let resp = client
        .get(format!("{base}/api/v1/counters"))
        .send()
        .await
        .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let counters: Counters = resp
        .json()
        .await
        .expect("failed to convert response to json");
    let counter_names: HashSet<_> = counters.counters.iter().map(|c| c.id.as_str()).collect();
    let expected_counter_names: HashSet<_> = [
        "totalBlocks",
        "averageBlockTime",
        "completedTxns",
        "totalTxns",
        "totalAccounts",
        "totalTokens",
        "totalNativeCoinHolders",
        "totalNativeCoinTransfers",
        "lastNewContracts",
        "lastNewVerifiedContracts",
    ]
    .into_iter()
    .collect();

    assert_eq!(counter_names, expected_counter_names);
}
