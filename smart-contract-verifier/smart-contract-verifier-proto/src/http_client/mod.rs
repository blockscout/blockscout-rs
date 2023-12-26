mod client;
mod config;

#[cfg(feature = "mock")]
pub mod mock;

pub use client::{
    solidity_verifier_client, sourcify_verifier_client, vyper_verifier_client, Client,
};
pub use config::{Config, ConfigBuilder};
pub use reqwest_middleware::Error;

pub type Result<T> = std::result::Result<T, reqwest_middleware::Error>;
