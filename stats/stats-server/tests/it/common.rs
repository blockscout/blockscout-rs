use std::{collections::HashMap, future::Future, path::PathBuf, str::FromStr, time::Duration};

use blockscout_service_launcher::{
    launcher::ConfigSettings,
    test_database::TestDbGuard,
    test_server::{get_test_server_settings, send_get_request},
};
use reqwest::{RequestBuilder, Response};
use stats_proto::blockscout::stats::v1 as proto_v1;
use stats_server::{
    Settings,
    auth::{API_KEY_NAME, ApiKey},
};
use tokio::{
    task::JoinSet,
    time::{error::Elapsed, sleep},
};
use url::Url;
use wiremock::MockServer;

pub fn setup_single_key(settings: &mut Settings, key: ApiKey) {
    settings.api_keys = HashMap::from([("test_key".to_string(), key.key)]);
}

pub async fn send_arbitrary_request(request: RequestBuilder) -> Response {
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    if !response.status().is_success() {
        panic!("{}", response_panic_message(response).await);
    }
    response
}

pub(crate) async fn response_panic_message(response: Response) -> String {
    let status = response.status();
    let message = response.text().await.expect("Read body as text");
    format!("Invalid status code (success expected). Status: {status}. Message: {message}")
}

pub enum ChartSubset {
    Independent,
    #[allow(unused)]
    BlocksDependent,
    InternalTransactionsDependent,
    #[allow(unused)]
    UserOpsDependent,
    AllCharts,
}

pub async fn wait_for_subset_to_update(base: &Url, subset: ChartSubset) {
    wait_until(Duration::from_secs(300), || async {
        let statuses: proto_v1::UpdateStatus =
            send_get_request(base, "/api/v1/update-status").await;
        let matching_status = match subset {
            ChartSubset::Independent => statuses.independent_status(),
            ChartSubset::BlocksDependent => statuses.blocks_dependent_status(),
            ChartSubset::InternalTransactionsDependent => {
                statuses.internal_transactions_dependent_status()
            }
            ChartSubset::UserOpsDependent => statuses.user_ops_dependent_status(),
            ChartSubset::AllCharts => statuses.all_status(),
        };
        if matching_status == proto_v1::ChartSubsetUpdateStatus::CompletedInitialUpdate {
            return true;
        }
        false
    })
    .await
    .expect("Did not reach required indexing status in time");
}

pub async fn wait_until<F, Fut>(timeout: Duration, condition_fut: F) -> Result<(), Elapsed>
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    let wait_job = async {
        loop {
            let future = condition_fut();
            if future.await {
                return;
            }
            sleep(Duration::from_millis(250)).await;
        }
    };
    tokio::time::timeout(timeout, wait_job).await
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
    settings.indexer_db_url = Some(blockscout_db.db_url());
    settings.blockscout_api_url = Some(url::Url::from_str(&blockscout_api.uri()).unwrap());
    settings.enable_all_arbitrum = true;
    settings.enable_all_op_stack = true;
    settings.enable_all_eip_7702 = true;
    settings.multichain_mode = false;
    settings.metrics.enabled = false;
    settings.jaeger.enabled = false;
    // initialized separately to prevent errors (with `try_init`)
    settings.tracing.enabled = false;
    (settings, base)
}

pub async fn request_reupdate_from(
    base: &Url,
    key: &ApiKey,
    from: &str,
    charts: Vec<&str>,
) -> proto_v1::BatchUpdateChartsResult {
    let chart_names = charts.into_iter().map(|s| s.to_string()).collect();
    send_request_with_key(
        base,
        "/api/v1/charts/batch-update",
        reqwest::Method::POST,
        Some(&proto_v1::BatchUpdateChartsRequest {
            chart_names,
            from: Some(from.into()),
            update_later: None,
        }),
        key,
    )
    .await
}

pub async fn send_request_with_key<Response: for<'a> serde::Deserialize<'a>>(
    url: &Url,
    route: &str,
    method: reqwest::Method,
    payload: Option<&impl serde::Serialize>,
    key: &ApiKey,
) -> Response {
    let mut request = reqwest::Client::new().request(method, url.join(route).unwrap());
    if let Some(p) = payload {
        request = request.json(p);
    };
    request = request.header(API_KEY_NAME, &key.key);
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("Failed to send request"));

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.expect("Read body as text");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    response
        .json()
        .await
        .unwrap_or_else(|_| panic!("Response deserialization failed"))
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
        let result_string_start = format!("[{log_prefix}]: consolidated test ... ");
        match test_result {
            Ok(()) => println!("{result_string_start}ok"),
            Err(e) => {
                println!("{result_string_start}fail\nerror: {e}",);
                failed += 1;
            }
        }
    }
    let passed = total - failed;
    let msg = format!("[{log_prefix}]: {passed}/{total} consolidated tests passed");
    if failed > 0 {
        panic!("{msg}")
    } else {
        println!("{msg}")
    }
}

pub fn sorted_vec<T: Ord>(mut v: Vec<T>) -> Vec<T> {
    v.sort();
    v
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
