use reqwest::StatusCode;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid argument: {arg} - {error}")]
    InvalidArgument { arg: String, error: String },
    #[error("request related error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("sourcify returned 'Too Many Requests' error: {0}")]
    SourcifyTooManyRequests(String),
    #[error("sourcify returned 'Internal Server Error' error: {0}")]
    SourcifyInternalServerError(String),
    #[error("sourcify returned 'Not Found' error: {0}")]
    SourcifyNotFound(String),
    #[error("sourcify returned 'Bad Request' error: {0}")]
    SourcifyBadRequest(String),
    #[error("sourcify returned unexpected status code: {status_code} - {msg}")]
    SourcifyUnexpectedStatusCode {
        status_code: StatusCode,
        msg: String,
    },
}
