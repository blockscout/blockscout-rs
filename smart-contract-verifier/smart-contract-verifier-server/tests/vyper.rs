mod vyper_types;

use actix_web::{
    dev::ServiceResponse,
    http::StatusCode,
    test,
    test::{read_body, read_body_json, TestRequest},
    App,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::Deserialize;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    source::SourceType, verify_response::Status, vyper_verifier_actix::route_vyper_verifier,
    VerifyResponse,
};
use smart_contract_verifier_server::{Settings, VyperVerifierService};
use std::{
    str::{from_utf8, FromStr},
    sync::Arc,
};
use tokio::sync::{OnceCell, Semaphore};
use vyper_types::TestCase;

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

async fn test_setup(test_case: &impl TestCase) -> ServiceResponse {
    let service = global_service().await;
    let app = test::init_service(
        App::new().configure(|config| route_vyper_verifier(config, service.clone())),
    )
    .await;

    let request = test_case.to_request();

    TestRequest::post()
        .uri(ROUTE)
        .set_json(&request)
        .send_request(&app)
        .await
}

async fn test_success(test_case: impl TestCase) {
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
        verification_response.extra_data.is_some(),
        "Verification extra_data is absent"
    );

    let verification_result = verification_response
        .source
        .expect("Verification source is absent");

    let abi: Option<Result<ethabi::Contract, _>> = verification_result
        .abi
        .as_ref()
        .map(|abi| serde_json::from_str(abi));
    assert_eq!(
        verification_result.file_name,
        test_case.file_name(),
        "Invalid file name"
    );
    assert_eq!(
        verification_result.contract_name,
        test_case.contract_name(),
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
        verification_result_constructor_arguments,
        test_case.constructor_args(),
        "Invalid constructor args"
    );
    assert_eq!(
        verification_result.compiler_version,
        test_case.compiler_version(),
        "Invalid compiler version"
    );

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CompilerSettings {
        pub evm_version: Option<String>,
        pub optimize: Option<bool>,
        pub bytecode_metadata: Option<bool>,
    }
    let compiler_settings: CompilerSettings =
        serde_json::from_str(&verification_result.compiler_settings).unwrap_or_else(|_| {
            panic!(
                "Compiler Settings deserialization failed: {}",
                &verification_result.compiler_settings
            )
        });
    assert_eq!(
        compiler_settings.evm_version,
        test_case.evm_version(),
        "Invalid evm version"
    );
    assert_eq!(
        compiler_settings.optimize,
        test_case.optimize(),
        "Invalid optimize setting"
    );
    assert_eq!(
        compiler_settings.bytecode_metadata,
        test_case.bytecode_metadata(),
        "Invalid bytecode metadata settings"
    );

    assert_eq!(
        verification_result.source_files,
        test_case.source_files(),
        "Invalid source files"
    );
}

async fn test_failure(test_case: impl TestCase, expected_message: &str) {
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

async fn test_error(test_case: impl TestCase, expected_status: StatusCode, expected_message: &str) {
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
mod flattened {
    use super::{test_error, test_failure, test_success, vyper_types};
    use actix_web::http::StatusCode;
    use vyper_types::Flattened;

    #[tokio::test]
    async fn verify_success() {
        for test_case_name in &["simple", "arguments", "erc20", "erc667"] {
            let test_case = vyper_types::from_file::<Flattened>(test_case_name);
            test_success(test_case).await;
        }
    }

    #[tokio::test]
    async fn verify_fail() {
        let mut test_case = vyper_types::from_file::<Flattened>("arguments");
        test_case.source_code =
            "count: public(uint256)\n@external\ndef __init__():\n    self.count = 345678765"
                .to_string();
        test_failure(
            test_case,
            "No contract could be verified with provided data",
        )
        .await;

        let mut test_case = vyper_types::from_file::<Flattened>("erc20");
        test_case.creation_bytecode = "0x60".to_string();
        test_failure(
            test_case,
            "No contract could be verified with provided data",
        )
        .await;
    }

    #[tokio::test]
    async fn verify_error() {
        let mut test_case = vyper_types::from_file::<Flattened>("simple");
        test_case.compiler_version = "v0.1.400+commit.e67f0147".to_string();
        test_error(
            test_case.clone(),
            StatusCode::BAD_REQUEST,
            "Compiler version not found: ",
        )
        .await;

        let mut test_case = vyper_types::from_file::<Flattened>("simple");
        test_case.creation_bytecode = "0xkeklol".to_string();
        test_error(test_case, StatusCode::BAD_REQUEST, "Invalid bytecode: ").await;
    }
}

mod multi_part {
    use super::{test_success, vyper_types};
    use vyper_types::MultiPart;

    #[tokio::test]
    async fn verify_success() {
        for test_case_name in &["with_interfaces"] {
            let test_case = vyper_types::from_file::<MultiPart>(test_case_name);
            test_success(test_case).await;
        }
    }
}
