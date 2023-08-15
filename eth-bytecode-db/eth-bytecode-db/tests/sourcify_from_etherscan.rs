mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification::{
    sourcify_from_etherscan, sourcify_from_etherscan::VerificationRequest, Client, Error, Source,
    SourceType, VerificationMetadata,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    VerifyFromEtherscanSourcifyRequest, VerifyResponse,
};
use tonic::Response;
use verification_test_helpers::{
    smart_contract_veriifer_mock::{MockSourcifyVerifierService, SmartContractVerifierServer},
    VerifierService,
};

const DB_PREFIX: &str = "sourcify_from_etherscan";

#[async_trait]
impl VerifierService<VerificationRequest> for MockSourcifyVerifierService {
    type GrpcT = VerifyFromEtherscanSourcifyRequest;

    fn add_into_service(&mut self, request: Self::GrpcT, response: VerifyResponse) {
        self.expect_verify_from_etherscan()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }

    fn generate_request(
        &self,
        _id: u8,
        _metadata: Option<VerificationMetadata>,
    ) -> VerificationRequest {
        VerificationRequest {
            address: "0x1234".to_string(),
            chain: "77".to_string(),
        }
    }

    fn source_type(&self) -> SourceType {
        SourceType::Solidity
    }

    async fn verify(client: Client, request: VerificationRequest) -> Result<Source, Error> {
        sourcify_from_etherscan::verify(client, request).await
    }
}

#[fixture]
fn service() -> MockSourcifyVerifierService {
    MockSourcifyVerifierService::new()
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source(service: MockSourcifyVerifierService) {
    verification_test_helpers::test_returns_valid_source(DB_PREFIX, service).await
}
