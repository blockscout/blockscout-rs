use crate::helpers;
use blockscout_service_launcher::test_server;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn test_startup_works() {
    let config_file = helpers::create_temp_config(serde_json::json!({}));
    let base = helpers::init_proxy_verifier_server(|mut settings| {
        settings.chains_config = Some(config_file.as_ref().to_path_buf());
        settings
    })
    .await;
    let response: serde_json::Value = test_server::send_get_request(&base, "/health").await;
    assert_eq!(response, serde_json::json!({"status": "SERVING"}));
}
