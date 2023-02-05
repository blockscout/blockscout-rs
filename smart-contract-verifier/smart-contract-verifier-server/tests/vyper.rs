use actix_web::{
    dev::ServiceResponse,
    http::StatusCode,
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    source::SourceType, verify_response::Status, vyper_verifier_actix::route_vyper_verifier,
    BytecodeType, VerifyResponse,
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
const ROUTE: &str = "/api/v2/verifier/vyper/sources:verify-multi-part";

async fn global_service() -> &'static Arc<VyperVerifierService> {
    static SERVICE: OnceCell<Arc<VyperVerifierService>> = OnceCell::const_new();
    SERVICE
        .get_or_init(|| async {
            let settings = Settings::default();
            let compilers_lock = Semaphore::new(settings.compilers.max_threads.get());
            let service = VyperVerifierService::new(
                settings.vyper,
                Arc::new(compilers_lock),
                settings.extensions.vyper,
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
        let test_case_path = format!("{TEST_CASES_DIR}/{name}.json");
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
        "bytecode": test_case.creation_bytecode,
        "bytecodeType": BytecodeType::CreationInput.as_str_name(),
        "compilerVersion": test_case.compiler_version,
        "sourceFiles": {
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
        panic!("Invalid status code (success expected). Status: {status}. Messsage: {message}")
    }

    let verification_response: VerifyResponse = read_body_json(response).await;
    assert_eq!(
        verification_response.status(),
        Status::Success,
        "Invalid verification status. Response: {verification_response:?}"
    );

    assert!(
        verification_response.source.is_some(),
        "Verification source is not Some"
    );
    assert!(
        verification_response.extra_data.is_some(),
        "Verification extra_data is not Some"
    );

    let verification_result = verification_response.source.expect("Checked above");

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
    assert_eq!(
        verification_result.source_type(),
        SourceType::Vyper,
        "Invalid source type"
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

    mod compiler_settings {
        use serde::Deserialize;

        #[derive(Default, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Optimizer {
            pub enabled: Option<bool>,
            pub runs: Option<i32>,
        }
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CompilerSettings {
        pub libraries: BTreeMap<String, BTreeMap<String, String>>,
        #[serde(default)]
        pub optimizer: compiler_settings::Optimizer,
    }
    let compiler_settings: CompilerSettings =
        serde_json::from_str(&verification_result.compiler_settings).unwrap_or_else(|_| {
            panic!(
                "Compiler Settings deserialization failed: {}",
                &verification_result.compiler_settings
            )
        });
    assert_eq!(
        compiler_settings.libraries,
        BTreeMap::new(),
        "Invalid contract libraries"
    );
    assert_eq!(
        compiler_settings.optimizer.enabled, None,
        "Invalid optimization"
    );
    assert_eq!(
        compiler_settings.optimizer.runs, None,
        "Invalid optimization runs"
    );
    assert_eq!(
        verification_result.source_files.len(),
        1,
        "Invalid number of sources"
    );
    assert_eq!(
        verification_result
            .source_files
            .into_iter()
            .next()
            .unwrap()
            .1,
        test_case.source_code,
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
        verification_response.status(),
        Status::Failure,
        "Invalid verification status. Response: {verification_response:?}"
    );

    assert!(
        verification_response.source.is_none(),
        "Failure verification source should be None"
    );
    assert!(
        verification_response.extra_data.is_none(),
        "Failure verification extra data should be None"
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

    assert_eq!(status, expected_status, "Invalid status code: {status}",);

    assert!(
        message.contains(expected_message),
        "Invalid message: {message}"
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
    test_error(test_case, StatusCode::BAD_REQUEST, "Invalid bytecode: ").await;
}
