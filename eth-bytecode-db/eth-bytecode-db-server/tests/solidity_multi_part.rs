mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    BytecodeType, VerifySolidityMultiPartRequest,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2 as smart_contract_verifier_v2;
use tonic::Response;
use verification_test_helpers::{
    smart_contract_verifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    VerifierService,
};

const TEST_SUITE_NAME: &str = "sourcify";

const ROUTE: &str = "/api/v2/verifier/solidity/sources:verify-multi-part";

#[async_trait]
impl VerifierService for MockSolidityVerifierService {
    fn add_into_service(&mut self, response: smart_contract_verifier_v2::VerifyResponse) {
        self.expect_verify_multi_part()
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }

    fn source_type(&self) -> verification::SourceType {
        verification::SourceType::Solidity
    }
}

#[fixture]
fn service() -> MockSolidityVerifierService {
    MockSolidityVerifierService::new()
}

#[rstest]
#[tokio::test]
#[timeout(std::time::Duration::from_secs(30))]
#[ignore = "Needs database to run"]
async fn test_returns_valid_source(service: MockSolidityVerifierService) {
    let default_request = VerifySolidityMultiPartRequest {
        bytecode: "".to_string(),
        bytecode_type: BytecodeType::CreationInput.into(),
        compiler_version: "".to_string(),
        evm_version: None,
        optimization_runs: None,
        source_files: Default::default(),
        libraries: Default::default(),
    };
    verification_test_helpers::test_returns_valid_source(
        TEST_SUITE_NAME,
        service,
        ROUTE,
        default_request,
    )
    .await;
}
