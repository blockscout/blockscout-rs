use pretty_assertions::assert_eq;
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use smart_contract_verifier_http::{run as run_http_server, Settings};

#[actix_rt::test]
async fn server_start() {
    let mut settings = Settings::default();
    settings.solidity.enabled = false;
    settings.sourcify.enabled = false;
    settings.metrics.enabled = true;
    let base = format!("http://{}", settings.server.addr);
    let metrics_base = format!("http://{}", settings.metrics.addr);
    let _server_handle = {
        let settings = settings.clone();
        tokio::spawn(async move { run_http_server(settings).await })
    };

    let retry_policy = ExponentialBackoff::builder()
        .build_with_total_retry_duration(std::time::Duration::from_secs(10));
    let client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();

    let resp = client
        .get(format!("{base}/health"))
        .send()
        .await
        .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!("{metrics_base}/metrics"))
        .send()
        .await
        .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    for s in &[
        "# TYPE smart_contract_verifier_http_requests_duration_seconds histogram",
        "smart_contract_verifier_http_requests_duration_seconds_bucket{endpoint=\"/health\",method=\"GET\",status=\"200\"",
    ] {
        assert!(body.contains(s), "body doesn't have string {s}:\n{body}");
    }
}
