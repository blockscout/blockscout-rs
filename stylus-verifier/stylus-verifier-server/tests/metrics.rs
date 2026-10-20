// SPDX-License-Identifier: LicenseRef-Blockscout

//! Runs alone in its process: the sandbox runner registers its metrics once, under the prefix in
//! effect when the first executor is built.

use blockscout_service_launcher::test_server::{get_test_server_settings, init_server};
use stylus_verifier_server::Settings;

#[tokio::test]
async fn sandbox_metrics_are_exported_with_the_service_prefix() {
    let image = |name: &str| format!("{name}@sha256:{}", "0".repeat(64));
    let mut settings = Settings::default();
    let (server_settings, base) = get_test_server_settings();
    settings.server = server_settings;
    settings.sandbox = serde_json::from_value(serde_json::json!({
        "docker": {
            "addr": "ssh://docker@sandbox.invalid",
            "runner_image": image("runner"),
        },
        "verifier": {
            "images": [
                {
                    "image": image("tools"),
                    "cargo_stylus_versions": ["0.6.2"],
                    "toolchains": ["1.80.0"],
                },
            ],
        },
    }))
    .unwrap();
    let metrics_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    settings.metrics.enabled = true;
    settings.metrics.addr = ([127, 0, 0, 1], metrics_port).into();
    settings.tracing.enabled = false;
    settings.jaeger.enabled = false;

    init_server(|| stylus_verifier_server::run(settings), &base).await;

    // Read what a scraper reads, not the registry: the launcher exports only its own registry.
    let exported = reqwest::get(format!("http://127.0.0.1:{metrics_port}/metrics"))
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    // Labelled runner metrics appear once a job sets a label; these exist from the start.
    for name in [
        "compiles_in_flight",
        "compiles_in_queue",
        "compile_time_seconds",
    ] {
        assert!(
            exported.contains(&format!("# TYPE stylus_verifier_{name} ")),
            "{name} is not exported: {exported}"
        );
        assert!(
            !exported.contains(&format!("# TYPE {name} ")),
            "{name} was exported without the service prefix: {exported}"
        );
    }
}
