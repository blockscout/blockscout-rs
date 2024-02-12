mod helpers;

use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_startup_works() {
    let base = helpers::init_proxy_verifier_server(|x| x).await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
