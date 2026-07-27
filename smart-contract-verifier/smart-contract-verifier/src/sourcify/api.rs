// SPDX-License-Identifier: LicenseRef-Blockscout

use super::{
    api_client::SourcifyApiClient,
    types::{Error, Success},
};
use anyhow::anyhow;
use blockscout_display_bytes::decode_hex;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub address: String,
    pub chain: String,
    pub files: BTreeMap<String, String>,
    /// Retained for API compatibility. The Sourcify v2 metadata endpoint infers
    /// the target contract from the supplied metadata, so this is unused.
    pub chosen_contract: Option<usize>,
}

pub async fn verify(
    sourcify_client: Arc<SourcifyApiClient>,
    request: VerificationRequest,
) -> Result<Success, Error> {
    let address = decode_hex(&request.address)
        .map_err(|err| {
            Error::BadRequest(anyhow!(
                "invalid contract address '{}': {err}",
                request.address
            ))
        })?
        .into();

    let (sources, metadata) = split_metadata_and_sources(request.files)?;

    let verified_contract = sourcify_client
        .lib_client()
        .verify_via_metadata_v2(request.chain.as_str(), address, sources, metadata, None)
        .await
        .map_err(error_handler::process_sourcify_error)?;

    Success::try_from(verified_contract)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyFromEtherscanRequest {
    pub address: bytes::Bytes,
    pub chain: String,
}

pub async fn verify_from_etherscan(
    sourcify_client: Arc<SourcifyApiClient>,
    request: VerifyFromEtherscanRequest,
) -> Result<Success, Error> {
    let verified_contract = sourcify_client
        .lib_client()
        .verify_from_etherscan_v2(request.chain.as_str(), request.address, None)
        .await
        .map_err(error_handler::process_sourcify_error)?;

    Success::try_from(verified_contract)
}

/// Splits the uploaded files into the Solidity metadata document and the
/// remaining source files, as required by the Sourcify v2 metadata endpoint.
///
/// The metadata file is identified by its content (a JSON object carrying the
/// `compiler`, `settings` and `output` keys) rather than by a fixed file name,
/// mirroring how Sourcify itself detected it under the v1 file-upload flow.
fn split_metadata_and_sources(
    files: BTreeMap<String, String>,
) -> Result<(BTreeMap<String, String>, serde_json::Value), Error> {
    let mut sources = BTreeMap::new();
    let mut metadata = None;

    for (name, content) in files {
        if metadata.is_none() {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                let looks_like_metadata = value.get("compiler").is_some()
                    && value.get("settings").is_some()
                    && value.get("output").is_some();
                if looks_like_metadata {
                    metadata = Some(value);
                    continue;
                }
            }
        }
        sources.insert(name, content);
    }

    let metadata = metadata.ok_or_else(|| {
        Error::BadRequest(anyhow!(
            "no contract metadata was found among the provided files"
        ))
    })?;

    Ok((sources, metadata))
}

mod error_handler {
    use super::Error;
    use sourcify::{EmptyCustomError, VerifyFromEtherscanError};

    // Is public just to make it possible to use it for generics in outer functions.
    // Implementations for required custom errors are supposed to be added inside this module.
    //
    // Added to avoid passing the handler inside `process_sourcify_error`.
    pub trait ErrorHandler: Sized {
        fn handle(self) -> Error;
    }

    impl ErrorHandler for EmptyCustomError {
        fn handle(self) -> Error {
            // Empty error cannot be initialized
            unreachable!()
        }
    }

    impl ErrorHandler for VerifyFromEtherscanError {
        fn handle(self) -> Error {
            match self {
                VerifyFromEtherscanError::ChainNotSupported(msg) => Error::Verification(msg),
                VerifyFromEtherscanError::TooManyRequests(msg) => {
                    Error::Internal(anyhow::anyhow!(msg))
                }
                VerifyFromEtherscanError::ApiResponseError(msg) => {
                    Error::Internal(anyhow::anyhow!(msg))
                }
                VerifyFromEtherscanError::ContractNotVerified(msg) => Error::Verification(msg),
                VerifyFromEtherscanError::CannotGenerateSolcJsonInput(msg) => {
                    Error::Verification(msg)
                }
                VerifyFromEtherscanError::VerifiedWithErrors(msg) => Error::Verification(msg),
            }
        }
    }

    pub fn process_sourcify_error<E: std::error::Error + ErrorHandler>(
        error: sourcify::Error<E>,
    ) -> Error {
        match error {
            sourcify::Error::Reqwest(_) | sourcify::Error::ReqwestMiddleware(_) => {
                Error::Internal(anyhow::anyhow!(error.to_string()))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::InternalServerError(_)) => {
                Error::Internal(anyhow::anyhow!(error.to_string()))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::NotFound(msg)) => {
                Error::BadRequest(anyhow::anyhow!("{msg}"))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::ChainNotSupported(msg)) => {
                Error::BadRequest(anyhow::anyhow!("{msg}"))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::VerificationFailure(msg)) => {
                Error::Verification(msg)
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::BadRequest(_)) => {
                tracing::error!(target: "sourcify", "{error}");
                Error::Internal(anyhow::anyhow!("{error}"))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::BadGateway(_)) => {
                tracing::error!(target: "sourcify", "{error}");
                Error::Internal(anyhow::anyhow!("{error}"))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::UnexpectedStatusCode { .. }) => {
                tracing::error!(target: "sourcify", "{error}");
                Error::Internal(anyhow::anyhow!("{error}"))
            }
            sourcify::Error::Sourcify(sourcify::SourcifyError::Custom(err)) => {
                tracing::error!(target: "sourcify", "custom endpoint error: {err}");
                E::handle(err)
            }
        }
    }
}
