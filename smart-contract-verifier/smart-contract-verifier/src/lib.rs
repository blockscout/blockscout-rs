pub mod solidity;
pub mod sourcify;
pub mod vyper;

mod common_types;
mod compiler;
mod consts;
mod lookup_methods;
mod metrics;
mod proto_conversions;
mod scheduler;
#[cfg(test)]
mod tests;
pub mod verify_new;
pub mod zksync;

pub(crate) use blockscout_display_bytes::Bytes as DisplayBytes;

pub use consts::{
    DEFAULT_ERA_SOLIDITY_COMPILER_LIST, DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST,
    DEFAULT_VYPER_COMPILER_LIST, DEFAULT_ZKSOLC_COMPILER_LIST,
};

pub use crate::sourcify::Error as SourcifyError;
pub use common_types::{
    Contract, FullyQualifiedName, Language, MatchType, OnChainCode, OnChainContract,
    RequestParseError,
};
pub use compiler::{
    CompactVersion, Compilers, DetailedVersion, Fetcher, FileValidator, ListFetcher, S3Fetcher,
    Version,
};

pub use crate::sourcify::{SourcifyApiClient, Success as SourcifySuccess};
pub use lookup_methods::{find_methods, LookupMethodsRequest, LookupMethodsResponse};
pub use solidity::{Client as SolidityClient, SolcValidator, SolidityCompiler};
pub use vyper::{Client as VyperClient, VyperCompiler};
