use tonic::{Response, Status};
use smart_contract_verifier::{BatchError, BatchVerificationResult, ZkBatchVerificationResult};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::zksync::solidity::{self, VerifyResponse, CompilationFailure};


pub fn compilation_error(message: impl Into<String>) -> Response<VerifyResponse> {
    Response::new(VerifyResponse {
        verify_response: Some(
            solidity::verify_response::VerifyResponse::CompilationFailure(
                CompilationFailure {
                    message: message.into(),
                },
            ),
        ),
    })
}

pub fn process_verification_result(
    result: ZkBatchVerificationResult,
) -> Result<Response<VerifyResponse>, Status> {
    let verification_result = match result {
        ZkBatchVerificationResult::Failure(_) => {
            solidity::verify_response::VerifyResponse::VerificationFailure(
                solidity::VerificationFailure {message: "No contract could be verified with provided data".into() },
            )
        }
        ZkBatchVerificationResult::Success(success) => {
            solidity::verify_response::VerifyResponse::VerificationSuccess(
                solidity::VerificationSuccess {}
            )
        }
    };

    Ok(Response::new(VerifyResponse {
        verify_response: Some(verification_result)
    }))
}

pub fn process_batch_error(
    error: BatchError,
) -> Result<Response<VerifyResponse>, Status> {
    match error {
        BatchError::VersionNotFound(_) => Err(Status::invalid_argument(error.to_string())),
        BatchError::Compilation(_) => {
            let response = compilation_error(error.to_string());
            Ok(response)
        }
        BatchError::Internal(_) => Err(Status::internal(error.to_string())),
    }
}