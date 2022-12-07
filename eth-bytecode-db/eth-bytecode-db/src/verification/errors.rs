use thiserror::Error;
use tonic::{Code, Status};

#[derive(Error, Debug)]
pub enum Error {
    #[error("Verification failed. Message - {message}")]
    VerificationFailed { message: String },
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Internal error: {0}")]
    Internal(anyhow::Error),
}

impl From<Status> for Error {
    fn from(status: Status) -> Self {
        match status.code() {
            Code::InvalidArgument => Self::InvalidArgument(status.message().to_string()),
            Code::Ok => Self::Internal(
                // should not happen, as we convert only errors
                anyhow::anyhow!("logical error: status of 'Ok' is processed when it must not to")
                    .context("verifier service connection"),
            ),
            code => Self::Internal(anyhow::anyhow!(code).context("verifier service connection")),
        }
    }
}
