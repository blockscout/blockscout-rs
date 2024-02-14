mod client;
mod config;
mod v2;

pub use client::Client;
pub use config::Config;
pub use reqwest::StatusCode;
pub use v2::*;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// There was an error running some middleware
    #[error("Middleware error: {0}")]
    Middleware(#[from] anyhow::Error),
    /// Error from the underlying reqwest client
    #[error("Request error: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("error decoding response body: {0}")]
    Decode(Box<dyn std::error::Error + Send + Sync>),
    #[error("Non-success status code: {}", .0.status())]
    StatusCode(reqwest::Response),
}

impl From<reqwest_middleware::Error> for Error {
    fn from(value: reqwest_middleware::Error) -> Error {
        match value {
            reqwest_middleware::Error::Middleware(err) => err.into(),
            reqwest_middleware::Error::Reqwest(err) => err.into(),
        }
    }
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> std::result::Result<T, D::Error>
where
    T: Default + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    let opt = <Option<T> as serde::Deserialize>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}
