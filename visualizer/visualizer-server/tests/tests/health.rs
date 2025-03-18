#[tokio::test]
async fn test_health_endpoint() {
    // Underlying `blockscout_service_launcher::init_server` uses health endpoint
    // to check that the service has started up. So, if init_server does not fail,
    // the health endpoint is working correctly.
    let _ = super::init_server().await;
}
