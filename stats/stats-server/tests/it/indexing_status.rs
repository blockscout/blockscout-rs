use blockscout_service_launcher::test_server::{init_server, send_get_request};
use itertools::Itertools;
use pretty_assertions::assert_eq;

use stats::tests::{init_db::init_db_all, mock_blockscout::mock_blockscout_api};
use stats_proto::blockscout::stats::v1::{
    health_check_response::ServingStatus, Counters, HealthCheckResponse,
};
use stats_server::stats;
use wiremock::ResponseTemplate;

use crate::common::{enabled_resolutions, get_test_stats_settings, send_arbitrary_request};

#[tokio::test]
#[ignore = "needs database"]
async fn test_not_indexed_ok() {
    // check that when the blockscout is not indexed, the healthcheck still succeeds and
    // charts don't have any points (i.e. they are not updated)
    let (stats_db, blockscout_db) = init_db_all("test_not_indexed_ok").await;
    let blockscout_api = mock_blockscout_api(ResponseTemplate::new(200).set_body_string(
        r#"{
            "finished_indexing": false,
            "finished_indexing_blocks": false,
            "indexed_blocks_ratio": "0.00",
            "indexed_internal_transactions_ratio": "0.00"
        }"#,
    ))
    .await;

    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // No update happens so we can wait less
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

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
    // returns onle counters with fallback query logic,
    // so they are returned even without calling an update
    assert_eq!(
        counters.counters.into_iter().map(|c| c.id).collect_vec(),
        vec!["totalAddresses", "totalBlocks", "totalTxns"]
    )
}
