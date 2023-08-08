mod client;
mod types;

pub use client::{Client, ClientBuilder};
pub use types::{
    GetSourceFilesResponse, MatchType, VerifyFromEtherscanError, VerifyFromEtherscanResponse,
};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SourcifyError<T> {
    #[error("'Internal Server Error': {0}")]
    InternalServerError(String),
    #[error("Chain is not supported: {0}")]
    ChainNotSupported(String),
    #[error("'Not Found': {0}")]
    NotFound(String),
    #[error("'Bad Request': {0}")]
    BadRequest(String),
    #[error("unexpected status code: {status_code} - {msg}")]
    UnexpectedStatusCode {
        status_code: reqwest::StatusCode,
        msg: String,
    },
    #[error("endpoint specific error: {0}")]
    Custom(T),
}

#[derive(Debug, thiserror::Error)]
pub enum Error<T> {
    #[error("invalid argument: {arg} - {error}")]
    InvalidArgument { arg: String, error: String },
    #[error("error occurred while sending request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("error with the middleware occurred while sending request: {0:#}")]
    ReqwestMiddleware(anyhow::Error),
    #[error("error got from the Sourcify: {0}")]
    Sourcify(#[from] SourcifyError<T>),
}
