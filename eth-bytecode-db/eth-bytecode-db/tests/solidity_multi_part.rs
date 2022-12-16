mod verification_test_helpers;

use crate::verification_test_helpers::VerifierServiceType;
use entity::sea_orm_active_enums;
use eth_bytecode_db::verification::{solidity_multi_part, solidity_multi_part::MultiPartFiles};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifySolidityMultiPartRequest,
};
use std::sync::Arc;
use tonic::Response;
use verification_test_helpers::smart_contract_veriifer_mock::MockSolidityVerifierService;

const DB_PREFIX: &str = "solidity_multi_part";

fn default_request_content() -> MultiPartFiles {
    MultiPartFiles {
        source_files: Default::default(),
        evm_version: "london".to_string(),
        optimization_runs: None,
        libraries: Default::default(),
    }
}

fn add_into_service(
    solidity_service: &mut MockSolidityVerifierService,
    request: VerifySolidityMultiPartRequest,
    response: VerifyResponse,
) {
    solidity_service
        .expect_verify_multi_part()
        .withf(move |arg| arg.get_ref() == &request)
        .returning(move |_| Ok(Response::new(response.clone())));
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source() {
    verification_test_helpers::returns_valid_source(
        DB_PREFIX,
        VerifierServiceType::Solidity {
            add_into_service: Arc::new(add_into_service),
        },
        default_request_content(),
        solidity_multi_part::verify,
    )
    .await
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn test_data_is_added_into_database() {
    verification_test_helpers::test_data_is_added_into_database(
        DB_PREFIX,
        VerifierServiceType::Solidity {
            add_into_service: Arc::new(add_into_service),
        },
        default_request_content(),
        solidity_multi_part::verify,
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
        "libraries": {},
        "optimization_runs": null,
        "source_files": {}
    });
    let verification_type = sea_orm_active_enums::VerificationType::MultiPartFiles;
    verification_test_helpers::historical_data_is_added_into_database(
        DB_PREFIX,
        VerifierServiceType::Solidity {
            add_into_service: Arc::new(add_into_service),
        },
        default_request_content(),
        solidity_multi_part::verify,
        verification_settings,
        verification_type,
    )
    .await;
}
