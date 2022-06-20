mod cli;
mod compiler;
mod config;
mod http_server;
mod solidity;
mod types;

#[cfg(test)]
mod tests;

pub use self::config::Config;
pub use http_server::{
    configure_router,
    handlers::verification::{VerificationResponse, VerificationResult, VerificationStatus},
    run as run_http_server, AppRouter, Router,
};
