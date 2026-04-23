use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_startup_works() {
    let server = crate::start_server().await;
    let response: serde_json::Value =
        test_server::send_get_request(&server.base_url, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
