mod verification_test_helpers;

use entity::sea_orm_active_enums;
use eth_bytecode_db::verification::{
    vyper_multi_part, vyper_multi_part::MultiPartFiles, SourceType, VerificationRequest,
};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifyVyperMultiPartRequest,
};
use tonic::Response;
use verification_test_helpers::{
    generate_verification_request,
    smart_contract_veriifer_mock::{MockVyperVerifierService, SmartContractVerifierServer},
    VerifierService,
};

const DB_PREFIX: &str = "vyper_multi_part";

fn default_request_content() -> MultiPartFiles {
    MultiPartFiles {
        source_files: Default::default(),
        evm_version: "london".to_string(),
        optimizations: false,
    }
}

impl VerifierService<VerifyVyperMultiPartRequest, VerificationRequest<MultiPartFiles>>
    for MockVyperVerifierService
{
    fn add_into_service(&mut self, request: VerifyVyperMultiPartRequest, response: VerifyResponse) {
        self.expect_verify_multi_part()
            .withf(move |arg| arg.get_ref() == &request)
            .returning(move |_| Ok(Response::new(response.clone())));
    }

    fn build_server(self) -> SmartContractVerifierServer {
        SmartContractVerifierServer::new().vyper_service(self)
    }

    fn generate_request(&self, id: u8) -> VerificationRequest<MultiPartFiles> {
        generate_verification_request(id, default_request_content())
    }

    fn source_type(&self) -> SourceType {
        SourceType::Vyper
    }
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source() {
    let service = MockVyperVerifierService::new();
    verification_test_helpers::returns_valid_source(DB_PREFIX, service, vyper_multi_part::verify)
        .await
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_data_is_added_into_database() {
    let service = MockVyperVerifierService::new();
    verification_test_helpers::test_data_is_added_into_database(
        DB_PREFIX,
        service,
        vyper_multi_part::verify,
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
        "evm_version": "london",
        "optimizations": false,
        "source_files": {}
    });
    let verification_type = sea_orm_active_enums::VerificationType::MultiPartFiles;
    let service = MockVyperVerifierService::new();
    verification_test_helpers::historical_data_is_added_into_database(
        DB_PREFIX,
        service,
        vyper_multi_part::verify,
        verification_settings,
        verification_type,
    )
    .await;
}
