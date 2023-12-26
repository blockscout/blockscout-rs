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

impl From<smart_contract_verifier_proto::http_client::Error> for Error {
    fn from(error: smart_contract_verifier_proto::http_client::Error) -> Self {
        match error {
            smart_contract_verifier_proto::http_client::Error::Reqwest(err) => {
                if let Some(status_code) = err.status() {
                    if status_code.is_client_error() {
                        return Self::InvalidArgument(err.to_string());
                    }
                }
                println!("\nALKFAFMAL: {err}\n:");
                Self::Internal(
                    anyhow::anyhow!(err.to_string()).context("verifier service connection"),
                )
            }
            smart_contract_verifier_proto::http_client::Error::Middleware(err) => {
                println!("\npklaksfafopok\n");
                Self::Internal(err.context("verifier service connection"))
            }
        }
    }
}
