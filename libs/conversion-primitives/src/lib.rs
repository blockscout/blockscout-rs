use thiserror::Error;

#[derive(Error, Debug)]
#[error("invalid argument: {0}")]
pub struct InvalidArgument(String);
impl InvalidArgument {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[derive(Error, Debug)]
#[error("internal error: {0}")]
pub struct InternalError(String);
impl InternalError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("internal error: {0}")]
    InternalError(String),
}

impl Error {
    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::InvalidArgument(message.into())
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self::InternalError(message.into())
    }
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
impl From<InvalidArgument> for tonic::Status {
    fn from(value: InvalidArgument) -> Self {
        tonic::Status::invalid_argument(value.0)
    }
}

#[cfg(feature = "tonic")]
impl From<InternalError> for tonic::Status {
    fn from(value: InternalError) -> Self {
        tonic::Status::internal(value.0)
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
