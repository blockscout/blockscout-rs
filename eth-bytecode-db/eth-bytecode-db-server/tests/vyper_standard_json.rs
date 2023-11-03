mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    BytecodeType, VerifyVyperStandardJsonRequest,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use tonic::Response;
use verification_test_helpers::{
    smart_contract_verifer_mock::{MockVyperVerifierService, SmartContractVerifierServer},
    test_cases, VerifierService,
};

const TEST_SUITE_NAME: &str = "vyper_standard_json";

const ROUTE: &str = "/api/v2/verifier/vyper/sources:verify-standard-json";

#[async_trait]
impl VerifierService<smart_contract_verifier_v2::VerifyResponse> for MockVyperVerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_standard_json()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().vyper_service(self)
    }
}

#[fixture]
fn service() -> MockVyperVerifierService {
    MockVyperVerifierService::new()
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_returns_valid_source(service: MockVyperVerifierService) {
    let default_request = VerifyVyperStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_returns_valid_source(
        TEST_SUITE_NAME,
        service,
        ROUTE,
        default_request,
        source_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_verify_then_search(service: MockVyperVerifierService) {
    let default_request = VerifyVyperStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_verify_then_search(
        TEST_SUITE_NAME,
        service,
        ROUTE,
        default_request,
        source_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_verify_same_source_twice(service: MockVyperVerifierService) {
    let default_request = VerifyVyperStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_verify_same_source_twice(
        TEST_SUITE_NAME,
        service,
        ROUTE,
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
    let default_request = VerifyVyperStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_search_returns_full_matches_only_if_any::<MockVyperVerifierService, _>(
        TEST_SUITE_NAME,
        ROUTE,
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
    let default_request = VerifyVyperStandardJsonRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        input: "".to_string(),
        metadata: None,
    };
    let source_type = verification::SourceType::Vyper;
    test_cases::test_update_source_then_search::<MockVyperVerifierService, _>(
        TEST_SUITE_NAME,
        ROUTE,
        default_request,
        source_type,
    )
    .await;
}
