use actix_web::{
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    solidity_verifier_actix::route_solidity_verifier, ListVersionsResponse,
};
use smart_contract_verifier_server::{Settings, SolidityVerifierService};
use std::{str::from_utf8, sync::Arc};
use tokio::sync::Semaphore;

async fn test_versions(uri: &str) {
    let settings = Settings::default();
    let compilers_lock = Semaphore::new(settings.compilers.max_threads.get());
    let solidity_service =
        SolidityVerifierService::new(settings.solidity, Arc::new(compilers_lock))
            .await
            .expect("couldn't initialize the app");

    let app = test::init_service(
        App::new().configure(|config| route_solidity_verifier(config, Arc::new(solidity_service))),
    )
    .await;

    let response = TestRequest::get().uri(uri).send_request(&app).await;

    // Assert that status code is success
    if !response.status().is_success() {
        let status = response.status();
        let body = read_body(response).await;
        let message = from_utf8(&body).expect("Read body as UTF-8");
        panic!(
            "Invalid status code (success expected). Status: {}. Message: {}",
            status, message
        )
    }

    let versions_response: ListVersionsResponse = read_body_json(response).await;
    assert!(
        !versions_response.versions.is_empty(),
        "List of versions is empty"
    )
}

#[tokio::test]
async fn solidity() {
    test_versions("/api/v1/solidity/versions").await;
}

// #[actix_rt::test]
// async fn vyper() {
//     test_versions("/api/v1/vyper/versions").await;
// }
