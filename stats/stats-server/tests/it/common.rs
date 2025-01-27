use std::{collections::HashMap, path::PathBuf, str::FromStr};

use blockscout_service_launcher::{
    launcher::ConfigSettings, test_database::TestDbGuard, test_server::get_test_server_settings,
};
use reqwest::{RequestBuilder, Response};
use stats_server::Settings;
use tokio::task::JoinSet;
use url::Url;
use wiremock::MockServer;

pub async fn send_arbitrary_request(request: RequestBuilder) -> Response {
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }
    response
}

pub async fn enabled_resolutions(
    line_charts: stats_proto::blockscout::stats::v1::LineCharts,
) -> HashMap<String, Vec<String>> {
    line_charts
        .sections
        .iter()
        .flat_map(|sec| sec.charts.clone())
        .map(|l| (l.id, l.resolutions))
        .collect()
}

pub fn get_test_stats_settings(
    stats_db: &TestDbGuard,
    blockscout_db: &TestDbGuard,
    blockscout_api: &MockServer,
) -> (Settings, Url) {
    let mut settings = Settings::build().expect("Failed to build settings");
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.charts_config = PathBuf::from_str("../config/charts.json").unwrap();
    settings.layout_config = PathBuf::from_str("../config/layout.json").unwrap();
    settings.update_groups_config = PathBuf::from_str("../config/update_groups.json").unwrap();
    settings.db_url = stats_db.db_url();
    settings.blockscout_db_url = blockscout_db.db_url();
    settings.blockscout_api_url = Some(url::Url::from_str(&blockscout_api.uri()).unwrap());
    settings.enable_all_arbitrum = true;
    (settings, base)
}

#[macro_export]
macro_rules! array_of_variables_with_names {
    ([
        $($var:ident),+ $(,)?
    ]) => {
        [
            $((stringify!($var), $var)),+
        ]
    };
}

pub async fn run_consolidated_tests(mut tests: JoinSet<()>, log_prefix: &str) {
    let mut failed = 0;
    let total = tests.len();
    println!("[{log_prefix}]: running {total} tests");
    while let Some(test_result) = tests.join_next().await {
        let result_string_start = format!("[{log_prefix}]: stats endpoint test ... ");
        match test_result {
            Ok(()) => println!("{result_string_start}ok"),
            Err(e) => {
                println!("{result_string_start}fail\nerror: {e}",);
                failed += 1;
            }
        }
    }
    let passed = total - failed;
    let msg = format!("[{log_prefix}]: {passed}/{total} not fully indexed endpoint tests passed");
    if failed > 0 {
        panic!("{}", msg)
    } else {
        println!("{}", msg)
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    #[test]
    fn array_of_variables_with_names_works() {
        let (var1, var2, var3, var4, var5) = (1, 2, 3, 4, 5);
        assert_eq!(
            array_of_variables_with_names!([var1, var2, var3, var4, var5]),
            [
                ("var1", var1),
                ("var2", var2),
                ("var3", var3),
                ("var4", var4),
                ("var5", var5),
            ]
        )
    }
}
