mod verification_test_helpers;

use eth_bytecode_db::verification::{sourcify, sourcify::VerificationRequest};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::{
    VerifyResponse, VerifyViaSourcifyRequest,
};
use std::sync::Arc;
use tonic::Response;
use verification_test_helpers::{
    smart_contract_veriifer_mock::MockSourcifyVerifierService, VerifierServiceType,
};

const DB_PREFIX: &str = "sourcify";

fn generate_verification_request(id: u8) -> VerificationRequest {
    VerificationRequest {
        address: "0x1234".to_string(),
        chain: "77".to_string(),
        chosen_contract: Some(id as i32),
        source_files: Default::default(),
    }
}

fn add_into_service(
    sourcify_service: &mut MockSourcifyVerifierService,
    request: VerifyViaSourcifyRequest,
    response: VerifyResponse,
) {
    sourcify_service
        .expect_verify()
        .withf(move |arg| arg.get_ref() == &request)
        .returning(move |_| Ok(Response::new(response.clone())));
}

#[tokio::test]
#[ignore = "Needs database to run"]
async fn returns_valid_source() {
    verification_test_helpers::returns_valid_source(
        DB_PREFIX,
        VerifierServiceType::Sourcify {
            add_into_service: Arc::new(add_into_service),
            generate_request: Arc::new(generate_verification_request),
        },
        sourcify::verify,
    )
    .await
}
