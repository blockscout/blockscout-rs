mod verification_test_helpers;

use async_trait::async_trait;
use entity::sea_orm_active_enums;
use eth_bytecode_db::verification::{
    solidity_standard_json, solidity_standard_json::StandardJson, Client, Error, Source,
    SourceType, VerificationMetadata, VerificationRequest,
};
use rstest::{fixture, rstest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
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

#[async_trait]
impl VerifierService<VerificationRequest<StandardJson>> for MockSolidityVerifierService {
    type GrpcT = VerifySolidityStandardJsonRequest;

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

    fn generate_request(
        &self,
        id: u8,
        metadata: Option<VerificationMetadata>,
    ) -> VerificationRequest<StandardJson> {
        generate_verification_request(id, default_request_content(), metadata)
    }

    fn source_type(&self) -> SourceType {
        SourceType::Solidity
    }

    async fn verify(
        client: Client,
        request: VerificationRequest<StandardJson>,
    ) -> Result<Source, Error> {
        solidity_standard_json::verify(client, request).await
    }
}

#[fixture]
fn service() -> MockSolidityVerifierService {
    MockSolidityVerifierService::new()
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_returns_valid_source(service: MockSolidityVerifierService) {
    verification_test_helpers::test_returns_valid_source(DB_PREFIX, service).await
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_data_is_added_into_database(service: MockSolidityVerifierService) {
    verification_test_helpers::test_data_is_added_into_database(DB_PREFIX, service).await
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_historical_data_is_added_into_database(service: MockSolidityVerifierService) {
    let verification_settings = serde_json::json!({
        "bytecode": "0x01",
        "bytecode_type": "CreationInput",
        "compiler_version": "compiler_version",
        "input": ""
    });
    let verification_type = sea_orm_active_enums::VerificationType::StandardJson;
    verification_test_helpers::test_historical_data_is_added_into_database(
        DB_PREFIX,
        service,
        verification_settings,
        verification_type,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_historical_data_saves_chain_id_and_contract_address(
    service: MockSolidityVerifierService,
) {
    verification_test_helpers::test_historical_data_saves_chain_id_and_contract_address(
        DB_PREFIX, service,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_verification_of_same_source_results_stored_once(
    service: MockSolidityVerifierService,
) {
    verification_test_helpers::test_verification_of_same_source_results_stored_once(
        DB_PREFIX, service,
    )
    .await;
}

#[rstest]
#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_verification_of_updated_source_replace_the_old_result() {
    verification_test_helpers::test_verification_of_updated_source_replace_the_old_result(
        DB_PREFIX, service,
    )
    .await;
}
