use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_server::{get_test_server_settings, init_server},
};
use pretty_assertions::assert_eq;

use stats::tests::{init_db::init_db_all, mock_blockscout::default_mock_blockscout_api};
use stats_server::{stats, Settings};

use std::{path::PathBuf, str::FromStr};

use crate::common::send_arbitrary_request;

#[tokio::test]
#[ignore = "needs database"]
async fn test_swagger_ok() {
    let (stats_db, blockscout_db) = init_db_all("test_swagger_ok").await;
    let blockscout_api = default_mock_blockscout_api().await;

    std::env::set_var("STATS__CONFIG", "./tests/config/test.toml");
    let mut settings = Settings::build().expect("Failed to build settings");
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.charts_config = PathBuf::from_str("../config/charts.json").unwrap();
    settings.layout_config = PathBuf::from_str("../config/layout.json").unwrap();
    settings.update_groups_config = PathBuf::from_str("../config/update_groups.json").unwrap();
    settings.db_url = stats_db.db_url();
    settings.blockscout_db_url = blockscout_db.db_url();
    settings.blockscout_api_url = Some(url::Url::from_str(&blockscout_api.uri()).unwrap());

    init_server(|| stats(settings), &base).await;

    // Sleep until server will start and calculate all values
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

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
