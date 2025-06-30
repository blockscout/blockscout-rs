mod verification_test_helpers;

use async_trait::async_trait;
use eth_bytecode_db::verification::{
    sourcify, sourcify::VerificationRequest, Client, Error, Source, SourceType,
    VerificationMetadata,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::{
    blockscout::smart_contract_verifier::v2::{VerifyResponse, VerifySourcifyRequest},
    http_client::mock::{MockSourcifyVerifierService, SmartContractVerifierServer},
};
use tonic::Response;
use verification_test_helpers::VerifierService;

const DB_PREFIX: &str = "sourcify";

#[async_trait]
impl VerifierService<VerificationRequest> for MockSourcifyVerifierService {
    type GrpcT = VerifySourcifyRequest;

    fn add_into_service(&mut self, request: VerifySourcifyRequest, response: VerifyResponse) {
        self.expect_verify()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }

    fn generate_request(
        &self,
        id: u8,
        _metadata: Option<VerificationMetadata>,
    ) -> VerificationRequest {
        VerificationRequest {
            address: "0x1234".to_string(),
            chain: "77".to_string(),
            chosen_contract: Some(id as i32),
            source_files: Default::default(),
        }
    }

    fn source_type(&self) -> SourceType {
        SourceType::Solidity
    }

    async fn verify(client: Client, request: VerificationRequest) -> Result<Source, Error> {
        sourcify::verify(client, request).await
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

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_verification_inserts_event_descriptions() {
    verification_test_helpers::test_verification_inserts_event_descriptions(DB_PREFIX, service)
        .await;
}
