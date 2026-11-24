// SPDX-License-Identifier: LicenseRef-Blockscout

//! A request that finds every build slot taken waits only while the queue has room and only for
//! the configured time; otherwise it gets 429. A slot is held by a request whose RPC endpoint
//! accepts the connection and never answers, so no Docker host is needed.

use blockscout_service_launcher::test_server::{get_test_server_settings, init_server};
use reqwest::StatusCode;
use std::time::{Duration, Instant};
use stylus_verifier_server::Settings;
use tokio::net::TcpListener;
use url::Url;

const ROUTE: &str = "/api/v1/stylus-sdk-rs:verify-github-repository";

async fn start_server(max_queued: usize, queue_timeout_seconds: u64) -> Url {
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
            "max_concurrent_verifications": 1,
            "max_queued_verifications": max_queued,
            "queue_timeout_seconds": queue_timeout_seconds,
            "allow_private_rpc_endpoints": true,
            "rpc_timeout_seconds": 60,
        },
    }))
    .unwrap();
    settings.metrics.enabled = false;
    settings.tracing.enabled = false;
    settings.jaeger.enabled = false;
    init_server(|| stylus_verifier_server::run(settings), &base).await;
    base
}

fn request(rpc_endpoint: &str) -> serde_json::Value {
    serde_json::json!({
        "deployment_transaction": format!("0x{}", "11".repeat(32)),
        "rpc_endpoint": rpc_endpoint,
        "cargo_stylus_version": "v0.6.2",
        "repository_url": "https://github.com/OffchainLabs/stylus-hello-world",
        "commit": "3a1bd232acece16c06b5295bc52e40d35165bb06",
        "path_prefix": "",
    })
}

async fn send(base: &Url, rpc_endpoint: &str) -> (StatusCode, String) {
    let response = reqwest::Client::new()
        .post(base.join(ROUTE).unwrap())
        .json(&request(rpc_endpoint))
        .send()
        .await
        .unwrap();
    (response.status(), response.text().await.unwrap())
}

/// Starts a request that takes the only slot and keeps it: it returns once the request is
/// inside, waiting for an RPC answer that never comes.
async fn occupy_the_slot(base: &Url) -> (tokio::task::JoinHandle<()>, TcpListener) {
    let rpc = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", rpc.local_addr().unwrap());
    let base = base.clone();
    let holder = tokio::spawn(async move {
        let _ = send(&base, &endpoint).await;
    });
    let (connection, _) = tokio::time::timeout(Duration::from_secs(30), rpc.accept())
        .await
        .expect("the first request never reached its RPC endpoint")
        .unwrap();
    // Kept open and unanswered for the rest of the test.
    std::mem::forget(connection);
    (holder, rpc)
}

#[tokio::test]
async fn a_full_queue_rejects_at_once() {
    let base = start_server(0, 300).await;
    let (holder, _rpc) = occupy_the_slot(&base).await;

    let started = Instant::now();
    let (status, body) = send(&base, "http://127.0.0.1:9").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("too many verifications"), "{body}");
    assert!(started.elapsed() < Duration::from_secs(5));
    holder.abort();
}

#[tokio::test]
async fn a_queued_request_gives_up_after_the_timeout() {
    let base = start_server(1, 2).await;
    let (holder, _rpc) = occupy_the_slot(&base).await;

    let started = Instant::now();
    let (status, body) = send(&base, "http://127.0.0.1:9").await;
    let waited = started.elapsed();
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body.contains("in time"), "{body}");
    assert!(
        waited >= Duration::from_secs(2) && waited < Duration::from_secs(10),
        "{waited:?}"
    );
    holder.abort();
}
