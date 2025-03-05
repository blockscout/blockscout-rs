//! Tests for the case
//! - blockscout is not indexed
//! - stats server is fully enabled but not updated (yet)
//!     (e.g. the update is slow and in progress)

use std::time::Duration;

use blockscout_service_launcher::{
    // launcher::GracefulShutdownHandler,
    launcher::GracefulShutdownHandler,
    test_server::{init_server, send_get_request},
};
use stats::tests::{
    init_db::init_db,
    mock_blockscout::{mock_blockscout_api, user_ops_status_response_json},
};
use stats_proto::blockscout::stats::v1::{
    health_check_response::ServingStatus, Counters, HealthCheckResponse,
};
use stats_server::stats;
use tokio::time::sleep;
use url::Url;
use wiremock::ResponseTemplate;

use crate::{
    common::{enabled_resolutions, get_test_stats_settings, send_arbitrary_request},
    it::mock_blockscout_simple::get_mock_blockscout,
};

pub async fn run_tests_with_charts_not_updated(variant: &str) {
    let test_name = &format!("run_tests_with_charts_not_updated_{}", variant);
    let _ = tracing_subscriber::fmt::try_init();
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
    let (mut settings, base) = get_test_stats_settings(&stats_db, blockscout_db, &blockscout_api);
    // will not update at all
    settings.force_update_on_start = None;
    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    // No update so no need to wait too long
    sleep(Duration::from_secs(3)).await;

    test_lines_counters_not_updated_ok(base).await;
    // stats_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
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
    // returns counters with fallback query logic
    for counter in ["totalAddresses", "totalBlocks", "totalTxns"] {
        assert!(
            counters.counters.iter().any(|kek| kek.id == counter),
            "counter {} not found in returned counters ({:?})",
            counter,
            counters.counters
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_0() {
    run_tests_with_charts_not_updated("0").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_1() {
    run_tests_with_charts_not_updated("1").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_2() {
    run_tests_with_charts_not_updated("2").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_3() {
    run_tests_with_charts_not_updated("3").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_4() {
    run_tests_with_charts_not_updated("4").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_5() {
    run_tests_with_charts_not_updated("5").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_6() {
    run_tests_with_charts_not_updated("6").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_7() {
    run_tests_with_charts_not_updated("7").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_8() {
    run_tests_with_charts_not_updated("8").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_9() {
    run_tests_with_charts_not_updated("9").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_10() {
    run_tests_with_charts_not_updated("10").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_11() {
    run_tests_with_charts_not_updated("11").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_12() {
    run_tests_with_charts_not_updated("12").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_13() {
    run_tests_with_charts_not_updated("13").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_14() {
    run_tests_with_charts_not_updated("14").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_15() {
    run_tests_with_charts_not_updated("15").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_16() {
    run_tests_with_charts_not_updated("16").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_17() {
    run_tests_with_charts_not_updated("17").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_18() {
    run_tests_with_charts_not_updated("18").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_19() {
    run_tests_with_charts_not_updated("19").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_20() {
    run_tests_with_charts_not_updated("20").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_21() {
    run_tests_with_charts_not_updated("21").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_22() {
    run_tests_with_charts_not_updated("22").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_23() {
    run_tests_with_charts_not_updated("23").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_24() {
    run_tests_with_charts_not_updated("24").await
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs database"]
async fn run_tests_with_charts_not_updated_25() {
    run_tests_with_charts_not_updated("25").await
}
