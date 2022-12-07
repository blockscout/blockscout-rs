mod verification_test_helpers;

use crate::verification_test_helpers::VerifierServiceType;
use eth_bytecode_db::verification::{solidity_standard_json, solidity_standard_json::StandardJson};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifySolidityStandardJsonRequest,
};
use std::sync::Arc;
use tonic::Response;
use verification_test_helpers::smart_contract_veriifer_mock::MockSolidityVerifierService;

const DB_PREFIX: &str = "solidity_standard_json";

fn default_request_content() -> StandardJson {
    StandardJson {
        input: "".to_string(),
    }
}

fn add_into_service(
    solidity_service: &mut MockSolidityVerifierService,
    request: VerifySolidityStandardJsonRequest,
    response: VerifyResponse,
) {
    solidity_service
        .expect_verify_standard_json()
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
        solidity_standard_json::verify,
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
        solidity_standard_json::verify,
    )
    .await
}
