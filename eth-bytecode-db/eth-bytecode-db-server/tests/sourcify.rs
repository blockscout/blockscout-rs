mod verification_test_helpers;

use eth_bytecode_db::verification;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::VerifySourcifyRequest;
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::http_client::mock::MockSourcifyVerifierService;
use verification_test_helpers::test_cases;

const TEST_SUITE_NAME: &str = "sourcify";

#[fixture]
fn service() -> MockSourcifyVerifierService {
    MockSourcifyVerifierService::new()
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(60))]
#[ignore = "Needs database to run"]
async fn test_returns_valid_source(service: MockSourcifyVerifierService) {
    let default_request = VerifySourcifyRequest {
        address: "".to_string(),
        chain: "".to_string(),
        files: Default::default(),
        chosen_contract: None,
    };
    let source_type = verification::SourceType::Solidity;
    test_cases::test_returns_valid_source(TEST_SUITE_NAME, service, default_request, source_type)
        .await;
}
