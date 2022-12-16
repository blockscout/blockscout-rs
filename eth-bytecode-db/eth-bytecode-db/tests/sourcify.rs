mod verification_test_helpers;

use eth_bytecode_db::verification::{sourcify, sourcify::VerificationRequest, SourceType};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifyViaSourcifyRequest,
};
use tonic::Response;
use verification_test_helpers::{
    smart_contract_veriifer_mock::{MockSourcifyVerifierService, SmartContractVerifierServer},
    VerifierService,
};

const DB_PREFIX: &str = "sourcify";

impl VerifierService<VerifyViaSourcifyRequest, VerificationRequest>
    for MockSourcifyVerifierService
{
    fn add_into_service(&mut self, request: VerifyViaSourcifyRequest, response: VerifyResponse) {
        self.expect_verify()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().sourcify_service(self)
    }

    fn generate_request(&self, id: u8) -> VerificationRequest {
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
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source() {
    let service = MockSourcifyVerifierService::new();
    verification_test_helpers::returns_valid_source(DB_PREFIX, service, sourcify::verify).await
}
