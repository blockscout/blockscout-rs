mod config;
mod client;
#[cfg(feature = "mock")]
mod mock;

pub use reqwest_middleware::Error;
pub use config::{Config, ConfigBuilder};
pub use client::{Client, sourcify_verifier_client, vyper_verifier_client, solidity_verifier_client};

pub type Result<T> = std::result::Result<T, reqwest_middleware::Error>;
