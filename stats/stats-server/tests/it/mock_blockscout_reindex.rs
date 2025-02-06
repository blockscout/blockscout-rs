use std::{str::FromStr, time::Duration};

use blockscout_service_launcher::test_server::{init_server, send_get_request};
use chrono::NaiveDate;
use stats::tests::{
    init_db::init_db_all,
    mock_blockscout::{default_mock_blockscout_api, fill_mock_blockscout_data, imitate_reindex},
    simple_test::{chart_output_to_expected, map_str_tuple_to_owned},
};
use stats_proto::blockscout::stats::v1::{self as proto_v1, BatchUpdateChartsResult};
use stats_server::{auth::ApiKey, stats};
use tokio::time::sleep;
use url::Url;

use crate::common::{get_test_stats_settings, request_reupdate_from, setup_single_key};

/// Uses reindexing, so needs to be independent
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
    let api_key = ApiKey::from_str_infallible("123");
    setup_single_key(&mut settings, api_key.clone());
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
    #[allow(clippy::identity_op)]
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
    #[allow(clippy::identity_op)]
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
        base,
        &format!("/api/v1/lines/{line_name}?resolution={resolution}"),
    )
    .await;
    chart.chart.into_iter().filter(|p| p.value != "0").collect()
}
