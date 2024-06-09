pub mod solidity;
pub mod sourcify;
pub mod vyper;

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
pub mod zksync;

pub(crate) use blockscout_display_bytes::Bytes as DisplayBytes;

pub use consts::{
    DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST, DEFAULT_VYPER_COMPILER_LIST,
    DEFAULT_ZKSOLC_COMPILER_LIST,
};

pub use middleware::Middleware;

pub use crate::sourcify::Error as SourcifyError;
pub use batch_verifier::{BatchError, BatchMatch, BatchSuccess, BatchVerificationResult};
pub use common_types::{Contract, MatchType};
pub use compiler::{
    CompactVersion, Compilers, DetailedVersion, Fetcher, FileValidator, ListFetcher, S3Fetcher,
    Version,
};
pub use verifier::{BytecodePart, Error as VerificationError};

pub use crate::sourcify::{SourcifyApiClient, Success as SourcifySuccess};
pub use lookup_methods::{find_methods, LookupMethodsRequest, LookupMethodsResponse};
pub use solidity::{
    Client as SolidityClient, SolcValidator, SolidityCompiler, Success as SoliditySuccess,
};
pub use vyper::{Client as VyperClient, Success as VyperSuccess, VyperCompiler};

pub fn decode_hex(value: &str) -> Result<Vec<u8>, hex::FromHexError> {
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(value)
    }
}
