use std::num::NonZeroUsize;

use pretty_assertions::assert_eq;
use verification::{make_retrying_request, run_http_server, Config};

#[actix_rt::test]
async fn server_start() {
    let mut config = Config::default();
    config.solidity.enabled = false;
    config.sourcify.enabled = false;
    config.metrics.enabled = true;
    let base = format!("http://{}", config.server.addr);
    let metrics_base = format!("http://{}", config.metrics.addr);
    let _server_handle = {
        let config = config.clone();
        tokio::spawn(async move { run_http_server(config).await })
    };

    let sleep_between = Some(tokio::time::Duration::from_millis(100));
    let attempts = NonZeroUsize::new(20).unwrap();

    let resp = make_retrying_request(attempts, sleep_between, || {
        reqwest::get(format!("{base}/health"))
    })
    .await
    .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let resp = make_retrying_request(attempts, sleep_between, || {
        reqwest::get(format!("{metrics_base}/metrics"))
    })
    .await
    .expect("failed to connect to server");
    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    for s in vec![
        "# TYPE verification_http_requests_duration_seconds histogram",
        "verification_http_requests_duration_seconds_bucket{endpoint=\"/health\",method=\"GET\",status=\"200\"",
    ] {
        assert!(body.contains(s), "body doesn't have string {s}:\n{body}");
    }
}
