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
    settings.linked_stats = LinkedStatsSettings {
        base_url: Some(url::Url::from_str(&linked_server.uri()).unwrap()),
        timeout: 1_500,
        max_hops: 1,
    };

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

    let baseline_linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available")
        .len();

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

    let linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available");
    assert_eq!(linked_requests.len(), baseline_linked_requests + 1);
    assert_eq!(
        linked_requests
            .last()
            .and_then(|request| request.headers.get("x-stats-link-hop"))
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );

    let update_status: proto_v1::UpdateStatus =
        send_get_request(&base, "/api/v1/update-status").await;
    assert_eq!(
        proto_v1::ChartSubsetUpdateStatus::try_from(update_status.all_status).unwrap(),
        proto_v1::ChartSubsetUpdateStatus::Pending
    );

    let linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available");
    assert_eq!(linked_requests.len(), baseline_linked_requests + 2);
    assert_eq!(
        linked_requests
            .last()
            .and_then(|request| request.headers.get("x-stats-link-hop"))
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );

    let linked_line_chart: proto_v1::LineChart =
        send_get_request(&base, "/api/v1/lines/linkedMissingChart?resolution=DAY").await;
    assert_eq!(linked_line_chart, linked_chart);

    let linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available");
    assert_eq!(linked_requests.len(), baseline_linked_requests + 3);
    assert_eq!(
        linked_requests
            .last()
            .and_then(|request| request.headers.get("x-stats-link-hop"))
            .and_then(|value| value.to_str().ok()),
        Some("0")
    );

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

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
async fn linked_stats_merges_line_sections() {
    let test_name = "linked_stats_merges_line_sections";
    let _ = tracing_subscriber::fmt::try_init();

    let stats_db = init_db(test_name).await;
    let blockscout_db = init_populated_blockscout_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    let linked_server = MockServer::start().await;

    let linked_lines = proto_v1::LineCharts {
        sections: vec![
            proto_v1::LineChartSection {
                id: "transactions".to_string(),
                title: "Secondary Transactions".to_string(),
                charts: vec![
                    line_info("newTxns", "Secondary New Txns"),
                    line_info("linked_only_line", "Linked Only Line"),
                ],
            },
            proto_v1::LineChartSection {
                id: "linked-only".to_string(),
                title: "Linked Only".to_string(),
                charts: vec![line_info("linked_only_section_line", "Linked Section Line")],
            },
        ],
    };
    let linked_interchain = proto_v1::MainPageInterchainStats {
        total_interchain_messages: Some(counter("totalInterchainMessages", "7")),
        total_interchain_messages_sent: Some(counter("totalInterchainMessagesSent", "8")),
        total_interchain_messages_received: Some(counter("totalInterchainMessagesReceived", "9")),
    };

    Mock::given(method("GET"))
        .and(path("/api/v1/lines"))
        .respond_with(ResponseTemplate::new(200).set_body_json(linked_lines.clone()))
        .mount(&linked_server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v1/pages/interchain/main"))
        .respond_with(ResponseTemplate::new(200).set_body_json(linked_interchain.clone()))
        .mount(&linked_server)
        .await;

    let (mut settings, base) =
        get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api, None);
    settings.linked_stats = LinkedStatsSettings {
        base_url: Some(url::Url::from_str(&linked_server.uri()).unwrap()),
        timeout: 1_500,
        max_hops: 1,
    };

    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_until(Duration::from_secs(30), || {
        let base = base.clone();
        async move {
            reqwest::Client::new()
                .get(base.join("/api/v1/counters").unwrap())
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false)
        }
    })
    .await
    .expect("server did not become ready in time");

    let line_charts: proto_v1::LineCharts = send_get_request(&base, "/api/v1/lines").await;

    let transactions = line_charts
        .sections
        .iter()
        .find(|section| section.id == "transactions")
        .expect("transactions section should exist");
    assert_eq!(transactions.title, "Transactions");
    assert!(
        transactions
            .charts
            .iter()
            .any(|chart| chart.id == "newTxns" && chart.title != "Secondary New Txns")
    );
    assert_eq!(
        transactions
            .charts
            .iter()
            .filter(|chart| chart.id == "newTxns")
            .count(),
        1
    );
    assert!(
        transactions
            .charts
            .iter()
            .any(|chart| chart.id == "linked_only_line")
    );
    assert_eq!(line_charts.sections.last().unwrap().id, "linked-only");

    let interchain_page: proto_v1::MainPageInterchainStats =
        send_get_request(&base, "/api/v1/pages/interchain/main").await;
    assert_eq!(interchain_page, linked_interchain);

    stats_db.close_all_unwrap().await;
    blockscout_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
async fn linked_stats_falls_back_to_primary_when_secondary_fails_or_times_out() {
    let test_name = "linked_stats_falls_back_to_primary_when_secondary_fails_or_times_out";
    let _ = tracing_subscriber::fmt::try_init();

    let stats_db = init_db(test_name).await;
    let blockscout_db = init_populated_blockscout_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    let linked_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/counters"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&linked_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/pages/interchain/main"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(200))
                .set_body_json(proto_v1::MainPageInterchainStats {
                    total_interchain_messages: Some(counter("totalInterchainMessages", "7")),
                    total_interchain_messages_sent: Some(counter(
                        "totalInterchainMessagesSent",
                        "8",
                    )),
                    total_interchain_messages_received: Some(counter(
                        "totalInterchainMessagesReceived",
                        "9",
                    )),
                }),
        )
        .mount(&linked_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/api/v1/lines/linkedMissingChart"))
        .and(query_param("resolution", "DAY"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(200))
                .set_body_json(proto_v1::LineChart {
                    chart: vec![proto_v1::Point {
                        date: "2024-01-01".to_string(),
                        date_to: "2024-01-01".to_string(),
                        value: "7".to_string(),
                        is_approximate: false,
                    }],
                    info: Some(line_info("linkedMissingChart", "Linked Missing Chart")),
                }),
        )
        .mount(&linked_server)
        .await;

    let (mut settings, base) =
        get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api, None);
    settings.linked_stats = LinkedStatsSettings {
        base_url: Some(url::Url::from_str(&linked_server.uri()).unwrap()),
        timeout: 50,
        max_hops: 1,
    };

    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_until(Duration::from_secs(30), || {
        let base = base.clone();
        async move {
            reqwest::Client::new()
                .get(base.join("/api/v1/counters").unwrap())
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false)
        }
    })
    .await
    .expect("server did not become ready in time");

    let counters: proto_v1::Counters = send_get_request(&base, "/api/v1/counters").await;
    assert!(
        counters
            .counters
            .iter()
            .all(|counter| counter.id != "linkedOnlyCounter")
    );

    let interchain_page: proto_v1::MainPageInterchainStats =
        send_get_request(&base, "/api/v1/pages/interchain/main").await;
    assert_eq!(
        interchain_page,
        proto_v1::MainPageInterchainStats {
            total_interchain_messages: None,
            total_interchain_messages_sent: None,
            total_interchain_messages_received: None,
        }
    );

    let timed_out_line_response = reqwest::Client::new().get(
        base.join("/api/v1/lines/linkedMissingChart?resolution=DAY")
            .unwrap(),
    );
    let timed_out_line_response =
        send_arbitrary_request_allowing_failure(timed_out_line_response).await;
    assert_eq!(timed_out_line_response.status(), StatusCode::NOT_FOUND);

    stats_db.close_all_unwrap().await;
    blockscout_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
async fn linked_stats_stops_forwarding_when_hop_limit_is_reached() {
    let test_name = "linked_stats_stops_forwarding_when_hop_limit_is_reached";
    let _ = tracing_subscriber::fmt::try_init();

    let stats_db = init_db(test_name).await;
    let blockscout_db = init_populated_blockscout_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    let linked_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/counters"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(proto_v1::Counters {
                counters: vec![proto_v1::Counter {
                    id: "linkedOnlyCounter".to_string(),
                    value: "42".to_string(),
                    title: "Linked Only".to_string(),
                    units: None,
                    description: "Linked only counter".to_string(),
                }],
            }),
        )
        .mount(&linked_server)
        .await;

    let (mut settings, base) =
        get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api, None);
    settings.linked_stats = LinkedStatsSettings {
        base_url: Some(url::Url::from_str(&linked_server.uri()).unwrap()),
        timeout: 1_500,
        max_hops: 1,
    };

    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_until(Duration::from_secs(30), || {
        let base = base.clone();
        async move {
            reqwest::Client::new()
                .get(base.join("/api/v1/counters").unwrap())
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false)
        }
    })
    .await
    .expect("server did not become ready in time");

    let baseline_linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available")
        .len();

    let response = reqwest::Client::new()
        .get(base.join("/api/v1/counters").unwrap())
        .header("x-stats-link-hop", "0")
        .send()
        .await
        .expect("request should complete");
    assert!(response.status().is_success());
    let counters: proto_v1::Counters = response.json().await.expect("response should be JSON");
    assert!(
        counters
            .counters
            .iter()
            .all(|counter| counter.id != "linkedOnlyCounter")
    );
    let after = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available")
        .len();
    assert_eq!(after, baseline_linked_requests);

    stats_db.close_all_unwrap().await;
    blockscout_db.close_all_unwrap().await;
    shutdown.cancel_wait_timeout(None).await.unwrap();
}

#[tokio::test]
#[serial_test::serial]
#[ignore = "needs database"]
async fn linked_stats_caps_incoming_hop_header_to_hard_limit() {
    let test_name = "linked_stats_caps_incoming_hop_header_to_hard_limit";
    let _ = tracing_subscriber::fmt::try_init();

    let stats_db = init_db(test_name).await;
    let blockscout_db = init_populated_blockscout_db(test_name).await;
    let blockscout_api = default_mock_blockscout_api().await;
    let linked_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/v1/counters"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(proto_v1::Counters {
                counters: vec![proto_v1::Counter {
                    id: "linkedOnlyCounter".to_string(),
                    value: "42".to_string(),
                    title: "Linked Only".to_string(),
                    units: None,
                    description: "Linked only counter".to_string(),
                }],
            }),
        )
        .mount(&linked_server)
        .await;

    let (mut settings, base) =
        get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api, None);
    settings.linked_stats = LinkedStatsSettings {
        base_url: Some(url::Url::from_str(&linked_server.uri()).unwrap()),
        timeout: 1_500,
        max_hops: 100,
    };

    let shutdown = GracefulShutdownHandler::new();
    let shutdown_cloned = shutdown.clone();
    init_server(|| stats(settings, Some(shutdown_cloned)), &base).await;

    wait_until(Duration::from_secs(30), || {
        let base = base.clone();
        async move {
            reqwest::Client::new()
                .get(base.join("/api/v1/counters").unwrap())
                .send()
                .await
                .map(|response| response.status().is_success())
                .unwrap_or(false)
        }
    })
    .await
    .expect("server did not become ready in time");

    let baseline_linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available")
        .len();

    let response = reqwest::Client::new()
        .get(base.join("/api/v1/counters").unwrap())
        .header("x-stats-link-hop", "100")
        .send()
        .await
        .expect("request should complete");
    assert!(response.status().is_success());
    let _counters: proto_v1::Counters = response.json().await.expect("response should be JSON");

    let linked_requests = linked_server
        .received_requests()
        .await
        .expect("linked requests should be available");
    assert_eq!(linked_requests.len(), baseline_linked_requests + 1);
    assert_eq!(
        linked_requests
            .last()
            .and_then(|request| request.headers.get("x-stats-link-hop"))
            .and_then(|value| value.to_str().ok()),
        Some("3")
    );

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

fn line_info(id: &str, title: &str) -> proto_v1::LineChartInfo {
    proto_v1::LineChartInfo {
        id: id.to_string(),
        title: title.to_string(),
        description: format!("{title} description"),
        units: None,
        resolutions: vec!["DAY".to_string()],
    }
}

fn counter(id: &str, value: &str) -> proto_v1::Counter {
    proto_v1::Counter {
        id: id.to_string(),
        value: value.to_string(),
        title: id.to_string(),
        units: None,
        description: format!("{id} description"),
    }
}
