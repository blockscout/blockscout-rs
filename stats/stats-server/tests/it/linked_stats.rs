use std::{str::FromStr, time::Duration};

use blockscout_service_launcher::{
    launcher::GracefulShutdownHandler,
    test_database::TestDbGuard,
    test_server::{init_server, send_get_request},
};
use chrono::NaiveDate;
use pretty_assertions::assert_eq;
use reqwest::StatusCode;
use stats::tests::{
    init_db::{init_db, init_db_blockscout},
    mock_blockscout::{default_mock_blockscout_api, fill_mock_blockscout_data},
};
use stats_proto::blockscout::stats::v1 as proto_v1;
use stats_server::{LinkedStatsSettings, stats};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{method, path, query_param},
};

use crate::common::{get_test_stats_settings, wait_until};

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
async fn linked_stats_merge_and_line_fallback_work() {
    let test_name = "linked_stats_merge_and_line_fallback_work";
    let _ = tracing_subscriber::fmt::try_init();

    let stats_db = init_db(test_name).await;
    let blockscout_db = init_populated_blockscout_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    let linked_server = MockServer::start().await;

    let overlapping_counter = proto_v1::Counter {
        id: "averageBlockTime".to_string(),
        value: "999".to_string(),
        title: "secondary".to_string(),
        units: Some("seconds".to_string()),
        description: "secondary".to_string(),
    };
    let secondary_only_counter = proto_v1::Counter {
        id: "linkedOnlyCounter".to_string(),
        value: "42".to_string(),
        title: "Linked Only".to_string(),
        units: None,
        description: "Linked only counter".to_string(),
    };
    Mock::given(method("GET"))
        .and(path("/api/v1/counters"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(proto_v1::Counters {
                counters: vec![overlapping_counter.clone(), secondary_only_counter.clone()],
            }),
        )
        .mount(&linked_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/update-status"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(proto_v1::UpdateStatus {
                all_status: proto_v1::ChartSubsetUpdateStatus::Pending.into(),
                independent_status: proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate
                    .into(),
                blocks_dependent_status: proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate
                    .into(),
                internal_transactions_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                user_ops_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
                zetachain_cctx_dependent_status:
                    proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate.into(),
            }),
        )
        .mount(&linked_server)
        .await;

    let linked_chart = proto_v1::LineChart {
        chart: vec![proto_v1::Point {
            date: "2024-01-01".to_string(),
            date_to: "2024-01-01".to_string(),
            value: "7".to_string(),
            is_approximate: false,
        }],
        info: Some(proto_v1::LineChartInfo {
            id: "linkedMissingChart".to_string(),
            title: "Linked Missing Chart".to_string(),
            description: "Secondary chart".to_string(),
            units: None,
            resolutions: vec!["DAY".to_string()],
        }),
    };
    Mock::given(method("GET"))
        .and(path("/api/v1/lines/linkedMissingChart"))
        .and(query_param("resolution", "DAY"))
        .respond_with(ResponseTemplate::new(200).set_body_json(linked_chart.clone()))
        .mount(&linked_server)
        .await;

    let (mut settings, base) =
        get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api, None);
    settings.linked_stats = Some(LinkedStatsSettings {
        base_url: url::Url::from_str(&linked_server.uri()).unwrap(),
        timeout: 1_500,
    });

    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_until(Duration::from_secs(30), || {
        let base = base.clone();
        async move {
            let response = reqwest::Client::new()
                .get(base.join("/api/v1/counters").unwrap())
                .send()
                .await
                .ok();
            let Some(response) = response else {
                return false;
            };
            if !response.status().is_success() {
                return false;
            }
            let counters = response.json::<proto_v1::Counters>().await.ok();
            let Some(counters) = counters else {
                return false;
            };
            counters
                .counters
                .iter()
                .any(|counter| counter.id == "averageBlockTime")
        }
    })
    .await
    .expect("server did not become ready in time");

    let counters: proto_v1::Counters = send_get_request(&base, "/api/v1/counters").await;
    assert!(counters.counters.iter().any(|counter| {
        counter.id == secondary_only_counter.id && counter.value == secondary_only_counter.value
    }));
    let average_block_time = counters
        .counters
        .iter()
        .find(|counter| counter.id == overlapping_counter.id)
        .expect("primary counter should exist");
    assert_ne!(average_block_time.value, overlapping_counter.value);

    let update_status: proto_v1::UpdateStatus =
        send_get_request(&base, "/api/v1/update-status").await;
    assert_eq!(
        proto_v1::ChartSubsetUpdateStatus::try_from(update_status.all_status).unwrap(),
        proto_v1::ChartSubsetUpdateStatus::Pending
    );

    let linked_line_chart: proto_v1::LineChart =
        send_get_request(&base, "/api/v1/lines/linkedMissingChart?resolution=DAY").await;
    assert_eq!(linked_line_chart, linked_chart);

    let not_found_response = reqwest::Client::new().get(
        base.join("/api/v1/lines/unavailableEverywhere?resolution=DAY")
            .unwrap(),
    );
    let not_found_response = send_arbitrary_request_allowing_failure(not_found_response).await;
    assert_eq!(not_found_response.status(), StatusCode::NOT_FOUND);

    stats_db.close_all_unwrap().await;
    blockscout_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

async fn init_populated_blockscout_db(test_name: &str) -> TestDbGuard {
    let blockscout_db = init_db_blockscout(test_name).await;
    fill_mock_blockscout_data(&blockscout_db, NaiveDate::from_str("2023-03-01").unwrap()).await;
    blockscout_db
}

async fn send_arbitrary_request_allowing_failure(
    request: reqwest::RequestBuilder,
) -> reqwest::Response {
    request.send().await.expect("request should complete")
}
