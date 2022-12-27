use crate::{proto::VerifyResponse, types::VerifyResponseWrapper};
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
