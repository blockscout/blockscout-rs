pub mod solidity;
pub mod sourcify;
pub mod vyper;
pub mod zksolc;

pub mod middleware;

mod common_types;
mod compiler;
mod consts;
mod lookup_methods;
mod metrics;
mod scheduler;
mod verifier;

mod batch_verifier;
#[cfg(test)]
mod tests;
mod zksolc_standard_json;
mod zksync;

pub(crate) use blockscout_display_bytes::Bytes as DisplayBytes;

pub use consts::{
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
    DEFAULT_ZKSOLC_COMPILER_LIST,
};

pub use middleware::Middleware;

pub use crate::sourcify::Error as SourcifyError;
pub use batch_verifier::{BatchError, BatchMatch, BatchSuccess, BatchVerificationResult, ZkBatchSuccess, ZkBatchVerificationResult};
pub use common_types::{Contract, MatchType};
pub use compiler::{
    CompactVersion, Compilers, DetailedVersion, Fetcher, FileValidator, ListFetcher, S3Fetcher,
    Version,
};
pub use verifier::{BytecodePart, Error as VerificationError};

pub use crate::sourcify::{SourcifyApiClient, Success as SourcifySuccess};
pub use compiler::ZkSyncCompilers;
pub use lookup_methods::{find_methods, LookupMethodsRequest, LookupMethodsResponse};
pub use solidity::{
    Client as SolidityClient, SolcValidator, SolidityCompiler, Success as SoliditySuccess,
};
pub use vyper::{Client as VyperClient, Success as VyperSuccess, VyperCompiler};
pub use zksolc::ZkSolcCompiler;
