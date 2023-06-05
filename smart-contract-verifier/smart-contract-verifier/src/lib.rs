pub mod solidity;
pub mod sourcify;
pub mod vyper;

pub mod middleware;

mod common_types;
mod compiler;
mod consts;
mod metrics;
mod scheduler;
mod verifier;

#[cfg(test)]
mod tests;

pub(crate) use blockscout_display_bytes::Bytes as DisplayBytes;

pub use consts::{
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
};

pub use middleware::Middleware;

pub use common_types::MatchType;
pub use compiler::{Compilers, Fetcher, ListFetcher, S3Fetcher, Version};
pub use sourcify::Error as SourcifyError;
pub use verifier::{BytecodePart, Error as VerificationError};

pub use solidity::{
    Client as SolidityClient, SolcValidator, SolidityCompiler, Success as SoliditySuccess,
};
pub use sourcify::{SourcifyApiClient, Success as SourcifySuccess};
pub use vyper::{Client as VyperClient, Success as VyperSuccess, VyperCompiler};
