//! Tests for the case
//! - blockscout is not indexed
//! - stats server is fully enabled & updated as much as possible
//!     with not indexed blockscout

use std::time::Duration;

use blockscout_service_launcher::{test_database::TestDbGuard, test_server::init_server};
use futures::FutureExt;
use pretty_assertions::assert_eq;
use stats::tests::{init_db::init_db, mock_blockscout::mock_blockscout_api};
use stats_proto::blockscout::stats::v1 as proto_v1;
use stats_server::{
    auth::{ApiKey, API_KEY_NAME},
    stats,
};
use tokio::{task::JoinSet, time::sleep};
use url::Url;
use wiremock::ResponseTemplate;

use super::common_tests::{
    test_contracts_page_ok, test_counters_ok, test_lines_ok, test_main_page_ok,
    test_transactions_page_ok,
};
use crate::common::{
    get_test_stats_settings, healthcheck_successful, run_consolidated_tests,
    send_arbitrary_request, setup_single_key, wait_for_subset_to_update, ChartSubset,
};

pub async fn run_tests_with_charts_uninitialized(blockscout_db: TestDbGuard) {
    let test_name = "run_tests_with_charts_uninitialized";
    let stats_db = init_db(test_name).await;
    let blockscout_api = mock_blockscout_api(
        ResponseTemplate::new(200).set_body_string(
            r#"{
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "0.10",
                "indexed_internal_transactions_ratio": null
            }"#,
        ),
        // will error getting user ops status
        None,
    )
    .await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);
    // obviously don't use this anywhere except tests
    let api_key = ApiKey::from_str_infallible("123");
    setup_single_key(&mut settings, api_key.clone());

    init_server(
        || stats(settings),
        &base,
        None,
        Some(healthcheck_successful),
    )
    .await;

    // Sleep until server will start and calculate all values
    sleep(Duration::from_secs(8)).await;
    wait_for_subset_to_update(&base, ChartSubset::Independent).await;

    // these pages must be available right away to display users
    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base.clone()).boxed(),
        test_lines_ok(base.clone(), false, false).boxed(),
        test_counters_ok(base.clone(), false, false).boxed(),
        test_swagger_ok(base.clone()).boxed(),
        test_reupdate_without_correct_key_is_rejected(base).boxed(),
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

// #[tokio::test]
// #[ignore = "needs database"]
// pub async fn run_tests_with_user_ops_not_indexed() {
//     let test_name = "run_tests_with_user_ops_not_indexed";
//     let stats_db = init_db(test_name).await;
//     let blockscout_db = stats::tests::init_db::init_db_blockscout(test_name).await;
//     stats::tests::mock_blockscout::fill_mock_blockscout_data(
//         &blockscout_db,
//         <chrono::NaiveDate as std::str::FromStr>::from_str("2023-03-01").unwrap(),
//     )
//     .await;
//     let blockscout_api = mock_blockscout_api(
//         ResponseTemplate::new(200).set_body_string(
//             r#"{
//                 "finished_indexing": true,
//                 "finished_indexing_blocks": true,
//                 "indexed_blocks_ratio": "1.00",
//                 "indexed_internal_transactions_ratio": "1.00"
//             }"#,
//         ),
//         Some(ResponseTemplate::new(200).set_body_string(user_ops_status_response_json(false))),
//     )
//     .await;
//     std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
//     let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);
//     // settings.tracing.enabled = true;

//     println!("initing server");
//     init_server(
//         move || stats(settings),
//         &base,
//         Some(Duration::from_secs(60)),
//         Some(|_| async { true }),
//     )
//     .await;
//     sleep(Duration::from_secs(10)).await;
//     println!("waiting for subset update");
//     wait_for_subset_to_update(&base, ChartSubset::InternalTransactionsDependent).await;
//     sleep(Duration::from_secs(10)).await;

//     println!("testing");
//     // these pages must be available right away to display users
//     let tests: JoinSet<_> = [
//         test_lines_ok(base.clone(), true, false).boxed(),
//         test_counters_ok(base.clone(), true, false).boxed(),
//     ]
//     .into_iter()
//     .collect();
//     run_consolidated_tests(tests, test_name).await;
// }

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
