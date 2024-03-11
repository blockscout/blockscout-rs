mod batch_solidity_types;

use actix_web::{
    dev::ServiceResponse,
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use batch_solidity_types::{TestCaseRequest, TestCaseResponse};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    solidity_verifier_actix::route_solidity_verifier, BatchVerifyResponse,
};
use smart_contract_verifier_server::{Settings, SolidityVerifierService};
use std::{str::from_utf8, sync::Arc};
use tokio::sync::{OnceCell, Semaphore};

async fn global_service() -> &'static Arc<SolidityVerifierService> {
    static SERVICE: OnceCell<Arc<SolidityVerifierService>> = OnceCell::const_new();
    SERVICE
        .get_or_init(|| async {
            let settings = Settings::default();
            let compilers_lock = Semaphore::new(settings.compilers.max_threads.get());
            let service = SolidityVerifierService::new(
                settings.solidity,
                Arc::new(compilers_lock),
                settings.extensions.solidity,
            )
            .await
            .expect("couldn't initialize the service");
            Arc::new(service)
        })
        .await
}

async fn test_setup<Request: TestCaseRequest>(test_case: &Request) -> ServiceResponse {
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_solidity_verifier(config, service.clone())),
    )
    .await;

    TestRequest::post()
        .uri(Request::route())
        .set_json(&test_case.to_request())
        .send_request(&app)
        .await
}

async fn test_success<Request: TestCaseRequest, Response: TestCaseResponse>(
    test_case_request: &Request,
    test_case_response: &Response,
) {
    let response = test_setup(test_case_request).await;
    if !response.status().is_success() {
        let status = response.status();
        let body = read_body(response).await;
        let message = from_utf8(&body).expect("Read body as UTF-8");
        panic!("Invalid status code (success expected). Status: {status}. Messsage: {message}")
    }

    let verification_response: BatchVerifyResponse = read_body_json(response).await;
    test_case_response.check(verification_response);
}

mod success_tests {
    use super::*;
    use crate::batch_solidity_types::ContractVerificationSuccess;
    use batch_solidity_types::{CompilationFailure, StandardJson};

    #[tokio::test]
    async fn basic() {
        let (test_case_request, test_case_response) =
            batch_solidity_types::from_file::<StandardJson, ContractVerificationSuccess>("basic");
        println!("SMBKSGFOPA: {test_case_response:?}");
        test_success(&test_case_request, &test_case_response).await;
    }

    #[tokio::test]
    async fn compilation_error() {
        let (test_case_request, test_case_response) = batch_solidity_types::from_file::<
            StandardJson,
            CompilationFailure,
        >("compilation_error");
        test_success(&test_case_request, &test_case_response).await;
    }

    #[tokio::test]
    async fn invalid_standard_json() {
        let (test_case_request, test_case_response) = batch_solidity_types::from_file::<
            StandardJson,
            CompilationFailure,
        >("invalid_standard_json");
        test_success(&test_case_request, &test_case_response).await;
    }
}
