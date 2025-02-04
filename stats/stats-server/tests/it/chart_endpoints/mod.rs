//! Test all chart endpoints.
//!
//! It is not done in separate tests in order
//! to simplify and optimize the server initialization
//! procedure (i.e. to not initialize the read-only
//! server each time).
//!
//! Note: all tests should be read only

use std::{collections::HashMap, str::FromStr, time::Duration};

use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server::{init_server, send_get_request},
};
use chrono::NaiveDate;
use contracts_page::test_contracts_page_ok;
use counters::test_counters_ok;
use futures::FutureExt;
use lines::test_lines_ok;
use main_page::test_main_page_ok;
use stats::tests::{
    init_db::{init_db, init_db_all, init_db_blockscout},
    mock_blockscout::{
        default_mock_blockscout_api, fill_mock_blockscout_data, imitate_reindex,
        mock_blockscout_api,
    },
    simple_test::{chart_output_to_expected, map_str_tuple_to_owned},
};
use stats_proto::blockscout::stats::v1::{self as proto_v1, BatchUpdateChartsResult};
use stats_server::{
    auth::{ApiKey, API_KEY_NAME},
    stats,
};
use tokio::{task::JoinSet, time::sleep};
use transactions_page::test_transactions_page_ok;
use url::Url;
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
    sleep(Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_lines_ok(base.clone()).boxed(),
        test_counters_ok(base.clone()).boxed(),
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base).boxed(),
        // todo: test cant reupdate w/o api key
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
    sleep(Duration::from_secs(8)).await;

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
    sleep(Duration::from_secs(8)).await;

    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), false).boxed(),
        test_transactions_page_ok(base, false).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

#[tokio::test]
#[ignore = "needs database"]
async fn tests_reupdate_works() {
    let test_name = "tests_reupdate_works";
    let (stats_db, blockscout_db) = init_db_all(test_name).await;
    let max_date = NaiveDate::from_str("2023-03-01").unwrap();
    fill_mock_blockscout_data(&blockscout_db, max_date).await;
    let blockscout_api = default_mock_blockscout_api().await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);
    // obviously don't use this anywhere except tests
    let api_key =
        ApiKey::from_str("0000000000000000000000000000000000000000000000000000000000000010");
    settings.api_keys = HashMap::from([("test_key".to_string(), api_key.clone())]);
    // settings.tracing.enabled = true;
    let wait_multiplier = if settings.tracing.enabled { 3 } else { 1 };

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    sleep(Duration::from_secs(8 * wait_multiplier)).await;

    let data = get_new_txns(&base).await;
    assert_eq!(
        chart_output_to_expected(data),
        map_str_tuple_to_owned(vec![
            ("2022-11-09", "6"),
            ("2022-11-10", "14"),
            ("2022-11-11", "16"),
            ("2022-11-12", "6"),
            ("2022-12-01", "6"),
            ("2023-01-01", "1"),
            ("2023-02-01", "5"),
            ("2023-03-01", "1"),
        ])
    );
    imitate_reindex(&blockscout_db, max_date).await;

    // should reindex newTxns transitively
    let reupdate_response =
        request_reupdate_from(&base, &api_key, "2023-01-01", vec!["txnsGrowth"]).await;
    assert_eq!(
        reupdate_response,
        BatchUpdateChartsResult {
            total: 1,
            total_rejected: 0,
            accepted: vec!["txnsGrowth".to_string()],
            rejected: vec![]
        }
    );
    // wait to reupdate
    sleep(Duration::from_secs(1 * wait_multiplier)).await;

    let data = get_new_txns(&base).await;
    assert_eq!(
        chart_output_to_expected(data),
        map_str_tuple_to_owned(vec![
            ("2022-11-09", "6"),
            ("2022-11-10", "14"),
            ("2022-11-11", "16"),
            ("2022-11-12", "6"),
            ("2022-12-01", "6"),
            ("2023-01-01", "3"),
            ("2023-02-01", "5"),
            ("2023-03-01", "1"),
        ])
    );

    let reupdate_response =
        request_reupdate_from(&base, &api_key, "2022-11-11", vec!["newTxns"]).await;
    assert_eq!(
        reupdate_response,
        BatchUpdateChartsResult {
            total: 1,
            total_rejected: 0,
            accepted: vec!["newTxns".to_string()],
            rejected: vec![]
        }
    );
    // wait to reupdate
    sleep(Duration::from_secs(1 * wait_multiplier)).await;

    let data = get_new_txns(&base).await;
    assert_eq!(
        chart_output_to_expected(data),
        map_str_tuple_to_owned(vec![
            ("2022-11-09", "6"),
            ("2022-11-10", "14"),
            ("2022-11-11", "20"),
            ("2022-11-12", "6"),
            ("2022-12-01", "6"),
            ("2023-01-01", "3"),
            ("2023-02-01", "5"),
            ("2023-03-01", "1"),
        ])
    );

    let reupdate_response =
        request_reupdate_from(&base, &api_key, "2000-01-01", vec!["newTxns"]).await;
    assert_eq!(
        reupdate_response,
        BatchUpdateChartsResult {
            total: 1,
            total_rejected: 0,
            accepted: vec!["newTxns".to_string()],
            rejected: vec![]
        }
    );
    // need to wait longer as reupdating from year 2000
    sleep(Duration::from_secs(2 * wait_multiplier)).await;

    let data = get_new_txns(&base).await;
    assert_eq!(
        chart_output_to_expected(data),
        map_str_tuple_to_owned(vec![
            ("2022-11-09", "6"),
            ("2022-11-10", "16"),
            ("2022-11-11", "20"),
            ("2022-11-12", "6"),
            ("2022-12-01", "6"),
            ("2023-01-01", "3"),
            ("2023-02-01", "5"),
            ("2023-03-01", "1"),
        ])
    );
}

async fn get_new_txns(base: &Url) -> Vec<proto_v1::Point> {
    let line_name = "newTxns";
    let resolution = "DAY";
    let chart: proto_v1::LineChart = send_get_request(
        &base,
        &format!("/api/v1/lines/{line_name}?resolution={resolution}"),
    )
    .await;
    chart.chart.into_iter().filter(|p| p.value != "0").collect()
}

async fn request_reupdate_from(
    base: &Url,
    key: &ApiKey,
    from: &str,
    charts: Vec<&str>,
) -> proto_v1::BatchUpdateChartsResult {
    let chart_names = charts.into_iter().map(|s| s.to_string()).collect();
    send_request_with_key(
        &base,
        &format!("/api/v1/charts/batch-update"),
        reqwest::Method::POST,
        Some(&proto_v1::BatchUpdateChartsRequest {
            chart_names,
            from: Some(from.into()),
            update_later: None,
        }),
        key,
    )
    .await
}

async fn send_request_with_key<Response: for<'a> serde::Deserialize<'a>>(
    url: &Url,
    route: &str,
    method: reqwest::Method,
    payload: Option<&impl serde::Serialize>,
    key: &ApiKey,
) -> Response {
    let mut request = reqwest::Client::new().request(method, url.join(route).unwrap());
    if let Some(p) = payload {
        request = request.json(p);
    };
    request = request.header(API_KEY_NAME, &key.key);
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("Response deserialization failed"))
}
