pub mod solidity;
// pub mod sourcify;

mod compiler;
mod consts;

// TODO: to be extracted in a separate crate
mod mismatch;
mod scheduler;

#[cfg(test)]
mod tests;

pub(crate) use ethers_core::types::Bytes as DisplayBytes;

pub use consts::{DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_VYPER_COMPILER_LIST};

pub use compiler::{Compilers, Fetcher, ListFetcher, S3Fetcher, Version};
pub use solidity::{SolcValidator, SolidityCompiler, Success as VerificationSuccess};
