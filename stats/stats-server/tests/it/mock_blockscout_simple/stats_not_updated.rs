//! Tests for the case
//! - blockscout is not indexed
//! - stats server is fully enabled but not updated (yet)
//!     (e.g. the update is slow and in progress)

use std::sync::Arc;

use blockscout_service_launcher::test_server::{init_server, send_get_request};
use itertools::Itertools;
use stats::tests::{
    init_db::init_db,
    mock_blockscout::{mock_blockscout_api, user_ops_status_response_json},
};
use stats_proto::blockscout::stats::v1::{
    health_check_response::ServingStatus, Counters, HealthCheckResponse,
};
use stats_server::stats;
use tokio::sync::Notify;
use url::Url;
use wiremock::ResponseTemplate;

use crate::{
    common::{enabled_resolutions, get_test_stats_settings, send_arbitrary_request},
    it::mock_blockscout_simple::get_mock_blockscout,
};

#[tokio::test]
#[ignore = "needs database"]
pub async fn run_tests_with_charts_not_updated() {
    let test_name = "run_tests_with_charts_not_updated";
    let stats_db = init_db(test_name).await;
    let blockscout_db = get_mock_blockscout().await;
    let blockscout_api = mock_blockscout_api(
        ResponseTemplate::new(200).set_body_string(
            r#"{
                "finished_indexing": false,
                "finished_indexing_blocks": false,
                "indexed_blocks_ratio": "0.00",
                "indexed_internal_transactions_ratio": "0.00"
            }"#,
        ),
        Some(ResponseTemplate::new(200).set_body_string(user_ops_status_response_json(false))),
    )
    .await;
    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (mut settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    // will not update at all
    settings.force_update_on_start = None;
    let stats_init_finished = Arc::new(Notify::new());
    let notify_handle = stats_init_finished.clone();
    init_server(move || stats(settings, Some(notify_handle)), &base).await;
    stats_init_finished.notified().await;

    test_lines_counters_not_updated_ok(base).await
}

pub async fn test_lines_counters_not_updated_ok(base: Url) {
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

    // check that charts return empty data, as they don't have any fallbacks
    // during update time

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
    // returns only counters with fallback query logic
    assert_eq!(
        counters.counters.into_iter().map(|c| c.id).collect_vec(),
        vec!["totalAddresses", "totalBlocks", "totalTxns",]
    )
}
