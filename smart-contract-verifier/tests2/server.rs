use std::num::NonZeroUsize;

use pretty_assertions::assert_eq;
use smart_contract_verifier::{make_retrying_request, run_http_server, Settings};

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

    let sleep_between = Some(tokio::time::Duration::from_millis(100));
    let attempts = NonZeroUsize::new(100).unwrap();

    let resp = make_retrying_request(attempts, sleep_between, || {
        reqwest::get(format!("{}/health", base))
    })
    .await
    .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let resp = make_retrying_request(attempts, sleep_between, || {
        reqwest::get(format!("{}/metrics", metrics_base))
    })
    .await
    .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    for s in vec![
        "# TYPE smart_contract_verifier_http_requests_duration_seconds histogram",
        "smart_contract_verifier_http_requests_duration_seconds_bucket{endpoint=\"/health\",method=\"GET\",status=\"200\"",
    ] {
        assert!(body.contains(s), "body doesn't have string {s}:\n{body}");
    }
}
