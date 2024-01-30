mod verification_test_helpers;

use eth_bytecode_db::verification;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    BytecodeType, VerifyVyperMultiPartRequest,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::http_client::mock::MockVyperVerifierService;
use verification_test_helpers::test_cases;

const TEST_SUITE_NAME: &str = "vyper_multi_part";

#[fixture]
fn service() -> MockVyperVerifierService {
    MockVyperVerifierService::new()
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_returns_valid_source(service: MockVyperVerifierService) {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_returns_valid_source(TEST_SUITE_NAME, service, default_request, source_type)
        .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_verify_then_search(service: MockVyperVerifierService) {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_verify_then_search(TEST_SUITE_NAME, service, default_request, source_type)
        .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_verify_same_source_twice(service: MockVyperVerifierService) {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_verify_same_source_twice(
        TEST_SUITE_NAME,
        service,
        default_request,
        source_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_search_returns_full_matches_only_if_any() {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_search_returns_full_matches_only_if_any::<MockVyperVerifierService, _>(
        TEST_SUITE_NAME,
        default_request,
        source_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_accepts_partial_verification_metadata_in_input() {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_accepts_partial_verification_metadata_in_input::<MockVyperVerifierService, _>(
        TEST_SUITE_NAME,
        default_request,
        source_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_update_source_then_search() {
    let default_request = VerifyVyperMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        source_files: Default::default(),
        interfaces: Default::default(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_update_source_then_search::<MockVyperVerifierService, _>(
        TEST_SUITE_NAME,
        default_request,
        source_type,
    )
    .await;
}
