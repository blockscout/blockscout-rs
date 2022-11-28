use actix_web::{
    dev::ServiceResponse,
    http::StatusCode,
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    vyper_verifier_actix::route_vyper_verifier, VerifyResponse,
};
use smart_contract_verifier_server::{Settings, VyperVerifierService};
use std::{
    collections::BTreeMap,
    fs,
    str::{from_utf8, FromStr},
    sync::Arc,
};
use tokio::sync::{OnceCell, Semaphore};

mod solidity_multiple_types;

const TEST_CASES_DIR: &str = "tests/test_cases_vyper";
const ROUTE: &str = "/api/v1/vyper/verify/multiple-files";

async fn global_service() -> &'static Arc<VyperVerifierService> {
    static SERVICE: OnceCell<Arc<VyperVerifierService>> = OnceCell::const_new();
    SERVICE
        .get_or_init(|| async {
            let settings = Settings::default();
            let compilers_lock = Semaphore::new(settings.compilers.max_threads.get());
            let service = VyperVerifierService::new(
                settings.vyper,
                Arc::new(compilers_lock),
                settings.extensions.solidity,
            )
            .await
            .expect("couldn't initialize the service");
            Arc::new(service)
        })
        .await
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TestCase {
    #[serde(default = "default_contract_name")]
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub source_code: String,
    pub expected_constructor_argument: Option<DisplayBytes>,
}

fn default_contract_name() -> String {
    "main".to_string()
}

impl TestCase {
    fn from_name(name: &str) -> Self {
        let test_case_path = format!("{}/{}.json", TEST_CASES_DIR, name);
        let content = fs::read_to_string(test_case_path).expect("failed to read file");
        serde_json::from_str(&content).expect("invalid test case format")
    }
}

async fn test_setup(test_case: &TestCase) -> ServiceResponse {
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_vyper_verifier(config, service.clone())),
    )
    .await;

    let request = serde_json::json!({
        "deployed_bytecode": test_case.deployed_bytecode,
        "creation_bytecode": test_case.creation_bytecode,
        "compiler_version": test_case.compiler_version,
        "sources": {
            format!("{}.vy", test_case.contract_name): test_case.source_code
        },
    });
    TestRequest::post()
        .uri(ROUTE)
        .set_json(&request)
        .send_request(&app)
        .await
}

async fn test_success(test_case: TestCase) {
    let response = test_setup(&test_case).await;
    if !response.status().is_success() {
        let status = response.status();
        let body = read_body(response).await;
        let message = from_utf8(&body).expect("Read body as UTF-8");
        panic!(
            "Invalid status code (success expected). Status: {}. Messsage: {}",
            status, message
        )
    }

    let verification_response: VerifyResponse = read_body_json(response).await;
    assert_eq!(
        verification_response.status,
        "0", // success
        "Invalid verification status. Response: {:?}",
        verification_response
    );

    assert!(
        verification_response.result.is_some(),
        "Verification result is not Some"
    );

    let verification_result = verification_response.result.expect("Checked above");

    let abi: Option<Result<ethabi::Contract, _>> = verification_result
        .abi
        .as_ref()
        .map(|abi| serde_json::from_str(abi));
    assert_eq!(
        verification_result.contract_name, test_case.contract_name,
        "Invalid contract name"
    );
    assert!(abi.is_some(), "Vyper contracts must have abi");
    assert!(
        abi.as_ref().unwrap().is_ok(),
        "Abi deserialization failed: {}",
        abi.unwrap().as_ref().unwrap_err()
    );
    let verification_result_constructor_arguments = verification_result
        .constructor_arguments
        .map(|args| DisplayBytes::from_str(&args).unwrap());
    assert_eq!(
        verification_result_constructor_arguments, test_case.expected_constructor_argument,
        "Invalid constructor args"
    );
    assert_eq!(
        verification_result.compiler_version, test_case.compiler_version,
        "Invalid compiler version"
    );
    assert_eq!(
        verification_result.contract_libraries,
        BTreeMap::new(),
        "Invalid contract libraries"
    );
    assert_eq!(
        verification_result.optimization, None,
        "Invalid optimization"
    );
    assert_eq!(
        verification_result.optimization_runs, None,
        "Invalid optimization runs"
    );
    assert_eq!(
        verification_result.sources.len(),
        1,
        "Invalid number of sources"
    );
    assert_eq!(
        verification_result.sources.values().next().unwrap(),
        &test_case.source_code,
        "Invalid source"
    );
}

async fn test_failure(test_case: TestCase, expected_message: &str) {
    let response = test_setup(&test_case).await;

    assert!(
        response.status().is_success(),
        "Invalid status code (success expected): {}",
        response.status()
    );

    let verification_response: VerifyResponse = read_body_json(response).await;

    assert_eq!(
        verification_response.status,
        "1", // failed
        "Invalid verification status. Response: {:?}",
        verification_response
    );

    assert!(
        verification_response.result.is_none(),
        "Failure verification result should be None"
    );

    assert!(
        verification_response.message.contains(expected_message),
        "Invalid message: {}",
        verification_response.message
    );
}

async fn test_error(test_case: TestCase, expected_status: StatusCode, expected_message: &str) {
    let response = test_setup(&test_case).await;
    let status = response.status();
    let body = read_body(response).await;
    let message = from_utf8(&body).expect("Read body as UTF-8");

    assert_eq!(status, expected_status, "Invalid status code: {}", status,);

    assert!(
        message.contains(expected_message),
        "Invalid message: {}",
        message
    );
}

#[tokio::test]
async fn vyper_verify_success() {
    for test_case_name in &["simple", "arguments", "erc20", "erc667"] {
        let test_case = TestCase::from_name(test_case_name);
        test_success(test_case).await;
    }
}

#[tokio::test]
async fn vyper_verify_fail() {
    let mut test_case = TestCase::from_name("arguments");
    test_case.source_code =
        "count: public(uint256)\n@external\ndef __init__():\n    self.count = 345678765"
            .to_string();
    test_failure(
        test_case,
        "No contract could be verified with provided data",
    )
    .await;

    let mut test_case = TestCase::from_name("erc20");
    test_case.creation_bytecode = "0x60".to_string();
    test_failure(
        test_case,
        "No contract could be verified with provided data",
    )
    .await;
}

#[tokio::test]
async fn vyper_verify_error() {
    let mut test_case = TestCase::from_name("simple");
    test_case.compiler_version = "v0.1.400+commit.e67f0147".to_string();
    test_error(
        test_case.clone(),
        StatusCode::BAD_REQUEST,
        "Compiler version not found: ",
    )
    .await;

    let mut test_case = TestCase::from_name("simple");
    test_case.creation_bytecode = "0xkeklol".to_string();
    test_error(
        test_case,
        StatusCode::BAD_REQUEST,
        "Invalid creation bytecode: ",
    )
    .await;
}
