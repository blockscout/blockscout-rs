pub mod solidity;
// pub mod sourcify;
pub mod vyper;

mod compiler;
mod consts;
mod verifier;

// TODO: to be extracted in a separate crate
mod mismatch;
mod scheduler;

#[cfg(test)]
mod tests;

pub(crate) use ethers_core::types::Bytes as DisplayBytes;

pub use consts::{DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_VYPER_COMPILER_LIST};

pub use compiler::{Compilers, Fetcher, ListFetcher, S3Fetcher, Version};
pub use verifier::{Error as VerificationError, Success as VerificationSuccess};

pub use solidity::{SolcValidator, SolidityCompiler};
// pub use sourcify::SourcifyApiClient;
pub use vyper::VyperCompiler;
