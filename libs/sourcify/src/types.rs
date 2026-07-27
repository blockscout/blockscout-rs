// SPDX-License-Identifier: LicenseRef-Blockscout

use serde::{Deserialize, Serialize};

pub(crate) use custom_error::CustomError;
mod custom_error {
    pub(crate) trait CustomError: std::error::Error + Sized {
        /// Maps a Sourcify API v2 error `customCode` (and its message) onto an
        /// endpoint-specific custom error. The same `customCode` may carry a
        /// different meaning depending on the endpoint (e.g. `unsupported_chain`
        /// is a verification failure for the Etherscan import endpoint, but a
        /// generic bad request elsewhere), so the interpretation is delegated to
        /// the concrete custom error type of the calling flow.
        fn handle_custom_code(_custom_code: &str, _message: &str) -> Option<Self> {
            None
        }
    }

    impl CustomError for super::EmptyCustomError {}
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmptyCustomError {}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchType {
    #[serde(alias = "perfect")]
    Full,
    Partial,
}

pub use verify_from_etherscan::VerifyFromEtherscanError;
mod verify_from_etherscan {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
    pub enum VerifyFromEtherscanError {
        // Is different from the common `ChainNotSupported` error in the way,
        // that may occur even if the chain is supported by the Sourcify in general,
        // but is not supported by Etherscan.
        #[error("{0}")]
        ChainNotSupported(String),
        #[error("{0}")]
        TooManyRequests(String),
        #[error("{0}")]
        ApiResponseError(String),
        #[error("{0}")]
        ContractNotVerified(String),
        #[error("{0}")]
        CannotGenerateSolcJsonInput(String),
        #[error("{0}")]
        VerifiedWithErrors(String),
    }

    impl CustomError for VerifyFromEtherscanError {
        // Sourcify API v2 reports Etherscan-import outcomes via `customCode`.
        // Preserve the v1 semantics: chain/verification issues surface as
        // verification failures, while rate limits and upstream API errors are
        // internal (retryable) errors.
        fn handle_custom_code(custom_code: &str, message: &str) -> Option<Self> {
            let message = message.to_string();
            match custom_code {
                "unsupported_chain" => Some(VerifyFromEtherscanError::ChainNotSupported(message)),
                // The recompiled bytecode did not match, or the contract is not
                // verified on the upstream Etherscan instance. `not_etherscan_verified`
                // is the code Sourcify actually returns for the latter (a `404`),
                // which must map to a verification failure rather than fall through
                // to the generic `404 -> NotFound` handling.
                "no_match"
                | "not_verified"
                | "contract_not_verified"
                | "not_etherscan_verified" => {
                    Some(VerifyFromEtherscanError::ContractNotVerified(message))
                }
                "compiler_error" | "verified_with_errors" => {
                    Some(VerifyFromEtherscanError::VerifiedWithErrors(message))
                }
                "cannot_generate_std_json_input" | "cannot_generate_solc_json_input" => Some(
                    VerifyFromEtherscanError::CannotGenerateSolcJsonInput(message),
                ),
                "too_many_requests" | "etherscan_limit" => {
                    Some(VerifyFromEtherscanError::TooManyRequests(message))
                }
                "etherscan_api_error" | "api_response_error" => {
                    Some(VerifyFromEtherscanError::ApiResponseError(message))
                }
                _ => None,
            }
        }
    }
}
