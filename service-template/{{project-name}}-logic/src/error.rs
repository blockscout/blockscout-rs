use thiserror::Error;
use tonic::{Code, Status};

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("failed to convert: {0}")]
    Convert(String),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}


impl From<ApiError> for Status {
    fn from(error: ApiError) -> Self {
        let code = match &error {
            ApiError::Convert(_) => Code::Internal,
            ApiError::Internal(_) => Code::Internal,
        };
        tonic::Status::new(code, error.to_string())
    }
}
