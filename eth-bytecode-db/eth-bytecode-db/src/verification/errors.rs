use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Verification failed. Message - {message}")]
    VerificationFailed { message: String },
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Internal error: {0}")]
    Internal(anyhow::Error),
    #[error("Verifier returned invalid response: {0}")]
    Verifier(anyhow::Error),
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
                Self::Internal(
                    anyhow::anyhow!(err.to_string()).context("verifier service connection"),
                )
            }
            smart_contract_verifier_proto::http_client::Error::Middleware(err) => {
                Self::Internal(err.context("verifier service connection"))
            }
            smart_contract_verifier_proto::http_client::Error::StatusCode(response) => {
                Self::Internal(anyhow::anyhow!(
                    "response returned with invalid status code; status={}",
                    response.status()
                ))
            }
        }
    }
}
