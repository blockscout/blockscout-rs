use actix_web::{
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use smart_contract_verifier_http::{configure_router, AppRouter, Settings, VersionsResponse};
use std::str::from_utf8;

async fn test_versions(uri: &str) {
    let mut settings = Settings::default();
    settings.sourcify.enabled = false;
    let app_router = AppRouter::new(settings)
        .await
        .expect("couldn't initialize the app");
    let app = test::init_service(App::new().configure(configure_router(&app_router))).await;

    let response = TestRequest::get().uri(uri).send_request(&app).await;

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let body = read_body(response).await;
        let message = from_utf8(&body).expect("Read body as UTF-8");
        panic!("Invalid status code (success expected). Status: {status}. Message: {message}")
    }

    let versions_response: VersionsResponse = read_body_json(response).await;
    assert!(
        !versions_response.versions.is_empty(),
        "List of versions is empty"
    )
}

#[actix_rt::test]
async fn solidity() {
    test_versions("/api/v1/solidity/versions").await;
}

#[actix_rt::test]
async fn vyper() {
    test_versions("/api/v1/vyper/versions").await;
}
