mod client;
mod types;

pub use client::{Client, ClientBuilder};
pub use types::{GetSourceFilesResponse, MatchType};

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SourcifyError {
    #[error("'Too Many Requests': {0}")]
    TooManyRequests(String),
    #[error("'Internal Server Error': {0}")]
    InternalServerError(String),
    #[error("'Not Found': {0}")]
    NotFound(String),
    #[error("'Bad Request': {0}")]
    BadRequest(String),
    #[error("unexpected status code: {status_code} - {msg}")]
    UnexpectedStatusCode {
        status_code: reqwest::StatusCode,
        msg: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid argument: {arg} - {error}")]
    InvalidArgument { arg: String, error: String },
    #[error("error occurred while sending request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("error with the middleware occurred while sending request: {0:#}")]
    ReqwestMiddleware(anyhow::Error),
    #[error("error got from the Sourcify: {0}")]
    Sourcify(#[from] SourcifyError),
}
