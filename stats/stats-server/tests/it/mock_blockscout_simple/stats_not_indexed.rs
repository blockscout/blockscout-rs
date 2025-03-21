//! Tests for the case
//! - blockscout is not indexed
//! - stats server is fully enabled & updated as much as possible
//!     with not indexed blockscout

use blockscout_service_launcher::{launcher::GracefulShutdownHandler, test_server::init_server};
use futures::FutureExt;
use pretty_assertions::assert_eq;
use stats::tests::{
    init_db::init_db,
    mock_blockscout::{mock_blockscout_api, user_ops_status_response_json},
};
use stats_proto::blockscout::stats::v1 as proto_v1;
use stats_server::{
    auth::{ApiKey, API_KEY_NAME},
    stats,
};
use tokio::task::JoinSet;
use url::Url;
use wiremock::ResponseTemplate;

use super::common_tests::{
    test_contracts_page_ok, test_counters_ok, test_lines_ok, test_main_page_ok,
    test_transactions_page_ok,
};
use crate::{
    common::{
        get_test_stats_settings, run_consolidated_tests, send_arbitrary_request, setup_single_key,
        wait_for_subset_to_update, ChartSubset,
    },
    it::mock_blockscout_simple::get_mock_blockscout,
};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_tests_with_nothing_indexed() {
    let test_name = "run_tests_with_nothing_indexed";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = mock_blockscout_api(
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "finished_indexing": false,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "0.10",
            "indexed_internal_transactions_ratio": null
        })),
        // will error getting user ops status
        None,
    )
    .await;
    let (blockscout_indexed, user_ops_indexed) = (false, false);
    let (mut settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    // obviously don't use this anywhere except tests
    let api_key = ApiKey::from_str_infallible("123");
    setup_single_key(&mut settings, api_key.clone());
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_for_subset_to_update(&base, ChartSubset::Independent).await;

    // these pages must be available right away to display users
    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), true, blockscout_indexed).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base.clone()).boxed(),
        test_lines_ok(base.clone(), blockscout_indexed, user_ops_indexed).boxed(),
        test_counters_ok(base.clone(), blockscout_indexed, user_ops_indexed).boxed(),
        test_swagger_ok(base.clone()).boxed(),
        test_reupdate_without_correct_key_is_rejected(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
    stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_tests_with_user_ops_not_indexed() {
    let test_name = "run_tests_with_user_ops_not_indexed";
    let _ = tracing_subscriber::fmt::try_init();
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = mock_blockscout_api(
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "finished_indexing": true,
            "finished_indexing_blocks": true,
            "indexed_blocks_ratio": "1.00",
            "indexed_internal_transactions_ratio": "1.00"
        })),
        Some(ResponseTemplate::new(200).set_body_json(user_ops_status_response_json(false))),
    )
    .await;
    let (blockscout_indexed, user_ops_indexed) = (true, false);
    let (settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_for_subset_to_update(&base, ChartSubset::InternalTransactionsDependent).await;

    let tests: JoinSet<_> = [
        test_lines_ok(base.clone(), blockscout_indexed, user_ops_indexed).boxed(),
        test_counters_ok(base.clone(), blockscout_indexed, user_ops_indexed).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
    stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

pub async fn test_swagger_ok(base: Url) {
    let request = reqwest::Client::new().request(
        reqwest::Method::GET,
        base.join("/api/v1/docs/swagger.yaml").unwrap(),
    );
    let response = send_arbitrary_request(request).await;
    let swagger_file_contents = response.text().await.unwrap();

    let expected_swagger_file_contents =
        tokio::fs::read_to_string("../stats-proto/swagger/stats.swagger.yaml")
            .await
            .unwrap();

    assert_eq!(swagger_file_contents, expected_swagger_file_contents,);
}

pub async fn test_reupdate_without_correct_key_is_rejected(base: Url) {
    let mut request = reqwest::Client::new().request(
        reqwest::Method::POST,
        base.join("/api/v1/charts/batch-update").unwrap(),
    );
    request = request.json(&proto_v1::BatchUpdateChartsRequest {
        chart_names: vec!["txnsGrowth".to_string()],
        from: Some("2023-01-01".to_string()),
        update_later: None,
    });
    let request_without_key = request.try_clone().unwrap();
    let response = request_without_key
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let request_with_incorrect_key = request.header(API_KEY_NAME, &"321".to_string());
    let response = request_with_incorrect_key
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));
    assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);
}
