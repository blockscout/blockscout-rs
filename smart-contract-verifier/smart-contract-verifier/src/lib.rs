// SPDX-License-Identifier: LicenseRef-Blockscout

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
mod verify;
pub mod zksync;

pub use consts::{
    DEFAULT_ERA_SOLIDITY_COMPILER_LIST, DEFAULT_SOLIDITY_COMPILER_LIST, DEFAULT_SOURCIFY_HOST,
    DEFAULT_VYPER_COMPILER_LIST, DEFAULT_ZKSOLC_COMPILER_LIST,
};

pub use common_types::{
    Contract, FullyQualifiedName, Language, MatchType, OnChainCode, OnChainContract,
    RequestParseError,
};
pub use compiler::{
    CommandArgument, CompactVersion, CompilerExecutor, CompilerInvocation,
    ConcurrencyLimitedCompilerExecutor, DetailedVersion, DockerCompilerExecutor,
    DockerCompilerExecutorSettings, ExecutionError, ExecutionOutput, Fetcher, FileValidator,
    JobFile, ListFetcher, NativeCompilerExecutor, S3Fetcher, Version,
    DEFAULT_COMPILER_EXECUTION_TIMEOUT_SECS, DEFAULT_COMPILER_MAX_OUTPUT_BYTES,
};
pub use verify::{
    Error, EvmCompilersPool, SolcCompiler, SolcInput, VerificationResult, VerifyingContract,
    VyperCompiler, VyperInput,
};

pub use lookup_methods::{find_methods, LookupMethodsRequest, LookupMethodsResponse};
pub use solidity::SolcValidator;
pub use sourcify::{Error as SourcifyError, SourcifyApiClient, Success as SourcifySuccess};
