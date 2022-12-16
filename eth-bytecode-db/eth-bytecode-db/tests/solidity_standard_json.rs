mod verification_test_helpers;

use entity::sea_orm_active_enums;
use eth_bytecode_db::verification::{
    solidity_standard_json, solidity_standard_json::StandardJson, SourceType, VerificationRequest,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifySolidityStandardJsonRequest,
};
use tonic::Response;
use verification_test_helpers::{
    generate_verification_request,
    smart_contract_veriifer_mock::{MockSolidityVerifierService, SmartContractVerifierServer},
    VerifierService,
};

const DB_PREFIX: &str = "solidity_standard_json";

fn default_request_content() -> StandardJson {
    StandardJson {
        input: "".to_string(),
    }
}

impl VerifierService<VerifySolidityStandardJsonRequest, VerificationRequest<StandardJson>>
    for MockSolidityVerifierService
{
    fn add_into_service(
        &mut self,
        request: VerifySolidityStandardJsonRequest,
        response: VerifyResponse,
    ) {
        self.expect_verify_standard_json()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().solidity_service(self)
    }

    fn generate_request(&self, id: u8) -> VerificationRequest<StandardJson> {
        generate_verification_request(id, default_request_content())
    }

    fn source_type(&self) -> SourceType {
        SourceType::Solidity
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source() {
    let service = MockSolidityVerifierService::new();
    verification_test_helpers::returns_valid_source(
        DB_PREFIX,
        service,
        solidity_standard_json::verify,
    )
    .await
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_data_is_added_into_database() {
    let service = MockSolidityVerifierService::new();
    verification_test_helpers::test_data_is_added_into_database(
        DB_PREFIX,
        service,
        solidity_standard_json::verify,
    )
    .await
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn historical_data_is_added_into_database() {
    let verification_settings = serde_json::json!({
        "bytecode": "0x01",
        "bytecode_type": "CreationInput",
        "compiler_version": "compiler_version",
        "input": ""
    });
    let verification_type = sea_orm_active_enums::VerificationType::StandardJson;
    let service = MockSolidityVerifierService::new();
    verification_test_helpers::historical_data_is_added_into_database(
        DB_PREFIX,
        service,
        solidity_standard_json::verify,
        verification_settings,
        verification_type,
    )
    .await;
}
