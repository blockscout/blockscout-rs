use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

#[cfg(feature = "tonic")]
cfg_if::cfg_if! {
    if #[cfg(feature = "tonic-0_8")] {
        use tonic_0_8 as tonic;
    } else {
        compile_error!(
            "one of the features ['tonic-0_8'] \
             must be enabled"
        );
    }
}

#[cfg(feature = "tonic")]
impl From<Error> for tonic::Status {
    fn from(value: Error) -> Self {
        match value {
            Error::InvalidArgument(message) => tonic::Status::invalid_argument(message),
            Error::InternalError(message) => tonic::Status::internal(message),
        }
    }
}
