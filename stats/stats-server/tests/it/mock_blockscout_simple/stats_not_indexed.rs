//! Tests for the case
//! - blockscout is not indexed
//! - stats server is fully enabled & updated as much as possible
//!     with not indexed blockscout

use std::time::Duration;

use blockscout_service_launcher::{
    test_database::TestDbGuard,
    test_server::{init_server, send_get_request},
};
use futures::FutureExt;
use itertools::Itertools;
use pretty_assertions::assert_eq;
use stats::tests::{init_db::init_db, mock_blockscout::mock_blockscout_api};
use stats_proto::blockscout::stats::v1::{
    health_check_response::ServingStatus, Counters, HealthCheckResponse,
};
use stats_server::stats;
use tokio::{task::JoinSet, time::sleep};
use url::Url;
use wiremock::ResponseTemplate;

use super::common_tests::{test_contracts_page_ok, test_main_page_ok, test_transactions_page_ok};
use crate::common::{
    enabled_resolutions, get_test_stats_settings, run_consolidated_tests, send_arbitrary_request,
};

// could be in `empty_charts` but let's keep it here for consistency
pub async fn run_tests_with_charts_uninitialized(blockscout_db: TestDbGuard) {
    let test_name = "run_tests_with_charts_uninitialized";
    let stats_db = init_db(test_name).await;
    let blockscout_api = mock_blockscout_api(ResponseTemplate::new(200).set_body_string(
        r#"{
            "finished_indexing": false,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "0.00",
            "indexed_internal_transactions_ratio": null
        }"#,
    ))
    .await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    sleep(Duration::from_secs(8)).await;

    // these pages must be available right away to display users
    let tests: JoinSet<_> = [
        test_main_page_ok(base.clone(), true).boxed(),
        test_transactions_page_ok(base.clone(), true).boxed(),
        test_contracts_page_ok(base.clone()).boxed(),
        test_lines_counters_not_indexed_ok(base.clone()).boxed(),
        test_swagger_ok(base).boxed(),
        // todo: test cant reupdate w/o api key
    ]
    .into_iter()
    .collect();
    run_consolidated_tests(tests, test_name).await;
}

pub async fn test_lines_counters_not_indexed_ok(base: Url) {
    // healthcheck is verified in `init_server`, but we double-check it just in case
    let request =
        reqwest::Client::new().request(reqwest::Method::GET, base.join("/health").unwrap());
    let response = send_arbitrary_request(request).await;

    assert_eq!(
        response.json::<HealthCheckResponse>().await.unwrap(),
        HealthCheckResponse {
            status: ServingStatus::Serving as i32
        }
    );

    // check that charts return empty data

    let enabled_resolutions =
        enabled_resolutions(send_get_request(&base, "/api/v1/lines").await).await;
    for (line_chart_id, resolutions) in enabled_resolutions {
        for resolution in resolutions {
            let chart: serde_json::Value = send_get_request(
                &base,
                &format!("/api/v1/lines/{line_chart_id}?resolution={resolution}"),
            )
            .await;
            let chart_data = chart
                .as_object()
                .expect("response has to be json object")
                .get("chart")
                .expect("response doesn't have 'chart' field")
                .as_array()
                .expect("'chart' field has to be json array");

            assert!(
                chart_data.is_empty(),
                "chart '{line_chart_id}' '{resolution}' is not empty"
            );
        }
    }

    let counters: Counters = send_get_request(&base, "/api/v1/counters").await;
    // returns only counters with fallback query logic,
    // so they are returned even without calling an update
    assert_eq!(
        counters.counters.into_iter().map(|c| c.id).collect_vec(),
        vec!["totalAddresses", "totalBlocks", "totalTxns"]
    )
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
