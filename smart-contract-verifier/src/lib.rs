pub mod solidity;
pub mod sourcify;
pub mod vyper;

mod compiler;
mod consts;
mod metrics;
mod middleware;
mod verifier;

// TODO: to be extracted in a separate crate
mod mismatch;
mod scheduler;

#[cfg(test)]
mod tests;

pub(crate) use ethers_core::types::Bytes as DisplayBytes;

pub use consts::{
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
};

pub use middleware::Middleware;

pub use compiler::{Compilers, Fetcher, ListFetcher, S3Fetcher, Version};
pub use sourcify::{Error as SourcifyError, Success as SourcifySuccess};
pub use verifier::{Error as VerificationError, Success as VerificationSuccess};

pub use solidity::{
    Client as SolidityClient, ClientBuilder as SolidityClientBuilder, SolcValidator,
    SolidityCompiler,
};
pub use sourcify::{SourcifyApiClient, SourcifyApiClientBuilder};
pub use vyper::{Client as VyperClient, ClientBuilder as VyperClientBuilder, VyperCompiler};
