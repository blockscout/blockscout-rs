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
            let response = VerifyResponseWrapper::ok(source);
            Ok(tonic::Response::new(response.into()))
        }
        Err(Error::VerificationFailed { message }) => {
            let response = VerifyResponseWrapper::err(message);
            Ok(tonic::Response::new(response.into()))
        }
        Err(Error::InvalidArgument(message)) => Err(tonic::Status::invalid_argument(message)),
        Err(Error::Internal(message)) => Err(tonic::Status::internal(message.to_string())),
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
