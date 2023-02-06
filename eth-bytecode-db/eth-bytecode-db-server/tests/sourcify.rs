mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::VerifySourcifyRequest;
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use tonic::Response;
use verification_test_helpers::{
    smart_contract_verifer_mock::{MockSourcifyVerifierService, SmartContractVerifierServer},
    test_cases, VerifierService,
};

const TEST_SUITE_NAME: &str = "sourcify";

const ROUTE: &str = "/api/v2/verifier/sourcify/sources:verify";

#[async_trait]
impl VerifierService<smart_contract_verifier_v2::VerifyResponse> for MockSourcifyVerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }
}

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
    test_cases::test_returns_valid_source(
        TEST_SUITE_NAME,
        service,
        ROUTE,
        default_request,
        source_type,
    )
    .await;
}
