mod test_startup_works;
mod test_stylus_sdk_rs;
mod types_stylus_sdk_rs;

use blockscout_service_launcher::test_server;
use stylus_verifier_server::Settings;
use url::Url;

struct ServerMetadata {
    base_url: Url,
}

async fn start_server() -> ServerMetadata {
    let (settings, base_url) = {
        let mut settings = Settings::default();
        let (server_settings, base_url) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        (settings, base_url)
    };
    test_server::init_server(|| stylus_verifier_server::run(settings), &base_url).await;

    ServerMetadata { base_url }
}

async fn expect_post_request(
    expected_status: reqwest::StatusCode,
    url: &Url,
    route: &str,
    payload: &impl serde::Serialize,
) -> reqwest::Response {
    expect_annotated_request(
        expected_status,
        url,
        route,
        reqwest::Method::POST,
        Some(payload),
        None,
    )
    .await
}

async fn expect_annotated_request(
    expected_status: reqwest::StatusCode,
    url: &Url,
    route: &str,
    method: reqwest::Method,
    payload: Option<&impl serde::Serialize>,
    annotation: Option<&str>,
) -> reqwest::Response {
    let annotation = annotation.map(|v| format!("({v}) ")).unwrap_or_default();

    let mut request = reqwest::Client::new().request(method, url.join(route).unwrap());
    if let Some(p) = payload {
        request = request.json(p);
    };
    let response = request
        .send()
        .await
        .unwrap_or_else(|_| panic!("{annotation}Failed to send request"));

    // Assert that status code is success
    let status = response.status();
    if status != expected_status {
        let message = response.text().await.expect("Read body as text");
        panic!("({annotation})Invalid status code ({expected_status} expected). Status: {status}. Message: {message}")
    }

    response
}
