use crate::{
    proto::{ListCompilerVersionsResponse, VerifyResponse},
    types::VerifyResponseWrapper,
};
use eth_bytecode_db::verification::{Error, Source};

pub fn process_verification_result(
    result: Result<Source, Error>,
) -> Result<tonic::Response<VerifyResponse>, tonic::Status> {
    match result {
        Ok(source) => {
            tracing::info!("Request processed successfully");
            let response = VerifyResponseWrapper::ok(source);
            Ok(tonic::Response::new(response.into()))
        }
        Err(Error::VerificationFailed { message }) => {
            tracing::info!("Verification failed: {message}");
            let response = VerifyResponseWrapper::err(message);
            Ok(tonic::Response::new(response.into()))
        }
        Err(Error::InvalidArgument(message)) => {
            tracing::info!(details = message, "Invalid argument");
            Err(tonic::Status::invalid_argument(message))
        }
        Err(Error::Internal(message)) => {
            tracing::info!(details=%message, "Internal error");
            Err(tonic::Status::internal(message.to_string()))
        }
        Err(err) => {
            tracing::error!("Unexpected error");
            Err(tonic::Status::internal(format!("Unexpected error: {err}")))
        }
    }
}

pub fn process_batch_import_error(error: Error) -> tonic::Status {
    match error {
        Error::Internal(message) => {
            tracing::info!(details=%message, "Internal error");
            tonic::Status::internal(message.to_string())
        }
        Error::Verifier(message) => {
            tracing::info!(details=%message, "Internal error");
            tonic::Status::internal(format!("Verifier error: {}", message))
        }
        err => {
            tracing::error!("Unexpected error");
            tonic::Status::internal(format!("Unexpected error: {err}"))
        }
    }
}

pub fn process_compiler_versions_result(
    result: Result<Vec<String>, anyhow::Error>,
) -> Result<tonic::Response<ListCompilerVersionsResponse>, tonic::Status> {
    match result {
        Ok(versions) => Ok(tonic::Response::new(ListCompilerVersionsResponse {
            compiler_versions: versions,
        })),
        Err(err) => Err(tonic::Status::internal(err.to_string())),
    }
}
