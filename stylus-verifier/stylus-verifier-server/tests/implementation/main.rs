// SPDX-License-Identifier: LicenseRef-Blockscout

mod test_startup_works;
mod test_stylus_sdk_rs;
mod types_stylus_sdk_rs;

use blockscout_service_launcher::test_server;
use stylus_verifier_server::{SandboxSettings, Settings};
use url::Url;

/// The sandbox of the ignored end-to-end tests comes from `STYLUS_VERIFIER_TEST_SANDBOX`, a JSON
/// document of the `sandbox` settings section. Without it the server gets a host it never
/// contacts, which is enough for every request that is answered before a build starts.
fn sandbox_settings() -> SandboxSettings {
    if let Ok(settings) = std::env::var("STYLUS_VERIFIER_TEST_SANDBOX") {
        return serde_json::from_str(&settings).expect("invalid STYLUS_VERIFIER_TEST_SANDBOX");
    }
    let image = |name: &str| format!("{name}@sha256:{}", "0".repeat(64));
    serde_json::from_value(serde_json::json!({
        "docker": {
            "addr": "ssh://docker@sandbox.invalid",
            "runner_image": image("runner"),
        },
        "verifier": {
            "images": [
                {
                    "image": image("tools"),
                    "cargo_stylus_versions": ["0.5.0", "0.6.2"],
                    "toolchains": ["1.80.0"],
                },
            ],
        },
    }))
    .unwrap()
}

struct ServerMetadata {
    base_url: Url,
}

async fn start_server() -> ServerMetadata {
    let (settings, base_url) = {
        let mut settings = Settings::default();
        let (server_settings, base_url) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.sandbox = sandbox_settings();
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
        panic!(
            "({annotation})Invalid status code ({expected_status} expected). Status: {status}. Message: {message}"
        )
    }

    response
}
