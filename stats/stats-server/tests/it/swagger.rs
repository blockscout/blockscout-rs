use blockscout_service_launcher::test_server::init_server;
use pretty_assertions::assert_eq;

use stats::tests::{init_db::init_db_all, mock_blockscout::default_mock_blockscout_api};
use stats_server::stats;

use crate::common::{get_test_stats_settings, send_arbitrary_request};

#[tokio::test]
#[ignore = "needs database"]
async fn test_swagger_ok() {
    let (stats_db, blockscout_db) = init_db_all("test_swagger_ok").await;
    let blockscout_api = default_mock_blockscout_api().await;

    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let (settings, base) = get_test_stats_settings(&stats_db, &blockscout_db, &blockscout_api);

    init_server(|| stats(settings), &base).await;

    // Wait to make sure the server started
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

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
