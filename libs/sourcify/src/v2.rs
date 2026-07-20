// SPDX-License-Identifier: LicenseRef-Blockscout

//! Sourcify API v2 support.
//!
//! Unlike the (now sunset) v1 API, verification in v2 is asynchronous and
//! job-based: a submission returns a `verificationId`, whose status is then
//! polled until the job completes. Once a contract is verified, its sources and
//! metadata are fetched from the contract endpoint.
//!
//! The methods here encapsulate the whole submit → poll → fetch flow so that
//! callers keep a single `await` that returns a fully materialized
//! [`VerifiedContract`] (or a mapped error), mirroring the ergonomics of the
//! old synchronous v1 client.

use crate::{
    types::CustomError, Client, EmptyCustomError, Error, MatchType, SourcifyError,
    VerifyFromEtherscanError,
};
use blockscout_display_bytes::{decode_hex, ToHex};
use bytes::Bytes;
use reqwest::{Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::BTreeMap;

/// A fully materialized, verified contract as returned by the v2 contract
/// endpoint. Carries everything required to reconstruct a verification success
/// downstream (sources, metadata, constructor arguments and the match type).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedContract {
    pub chain_id: String,
    pub address: Bytes,
    pub match_type: MatchType,
    pub sources: BTreeMap<String, String>,
    pub metadata: serde_json::Value,
    /// `None` means the constructor arguments were not returned by Sourcify
    /// (though they may still exist), as opposed to an empty byte sequence.
    pub constructor_arguments: Option<Bytes>,
}

/// Status of an asynchronous verification job (`GET /v2/verify/{id}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerificationJob {
    pub is_job_completed: bool,
    #[serde(default)]
    pub verification_id: Option<String>,
    #[serde(default)]
    pub contract: Option<JobContract>,
    #[serde(default)]
    pub error: Option<JobError>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobContract {
    #[serde(rename = "match", default)]
    pub match_type: Option<String>,
    #[serde(default)]
    pub runtime_match: Option<String>,
    #[serde(default)]
    pub creation_match: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobError {
    pub custom_code: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub error_id: Option<String>,
}

impl Client {
    /// Imports a contract verified on an Etherscan(-alike) instance into
    /// Sourcify via API v2, waiting for the asynchronous job to complete and
    /// returning the resulting verified contract.
    ///
    /// `api_key`, when provided, is forwarded to Sourcify to use against the
    /// upstream Etherscan instance.
    pub async fn verify_from_etherscan_v2(
        &self,
        chain_id: &str,
        contract_address: Bytes,
        api_key: Option<String>,
    ) -> Result<VerifiedContract, Error<VerifyFromEtherscanError>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            #[serde(skip_serializing_if = "Option::is_none")]
            api_key: Option<String>,
        }

        let url = self.generate_url(&format!(
            "v2/verify/etherscan/{chain_id}/{}",
            ToHex::to_hex(&contract_address)
        ));
        let response = self
            .reqwest_client
            .post(url)
            .json(&Body { api_key })
            .send()
            .await
            .map_err(map_middleware_error)?;

        let submitted: SubmitResponse = process_v2_response(response).await?;
        let job = self
            .poll_verification_job(&submitted.verification_id)
            .await?;
        self.finalize_job(chain_id, contract_address, job).await
    }

    /// Verifies a contract from its Solidity metadata and sources via API v2,
    /// waiting for the asynchronous job to complete.
    ///
    /// If the contract is already verified, Sourcify responds with `409` and the
    /// existing verified contract is fetched and returned instead.
    pub async fn verify_via_metadata_v2(
        &self,
        chain_id: &str,
        contract_address: Bytes,
        sources: BTreeMap<String, String>,
        metadata: serde_json::Value,
        creation_transaction_hash: Option<String>,
    ) -> Result<VerifiedContract, Error<EmptyCustomError>> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body {
            sources: BTreeMap<String, String>,
            metadata: serde_json::Value,
            #[serde(skip_serializing_if = "Option::is_none")]
            creation_transaction_hash: Option<String>,
        }

        let url = self.generate_url(&format!(
            "v2/verify/metadata/{chain_id}/{}",
            ToHex::to_hex(&contract_address)
        ));
        let response = self
            .reqwest_client
            .post(url)
            .json(&Body {
                sources,
                metadata,
                creation_transaction_hash,
            })
            .send()
            .await
            .map_err(map_middleware_error)?;

        // The contract is already verified — return the stored result.
        if response.status() == StatusCode::CONFLICT {
            return self.get_contract_v2(chain_id, contract_address).await;
        }

        let submitted: SubmitResponse = process_v2_response(response).await?;
        let job = self
            .poll_verification_job(&submitted.verification_id)
            .await?;
        self.finalize_job(chain_id, contract_address, job).await
    }

    /// Polls `GET /v2/verify/{id}` until the job reports completion, bounded by
    /// [`Client::max_poll_attempts`] and [`Client::poll_interval`].
    async fn poll_verification_job<E: CustomError>(
        &self,
        verification_id: &str,
    ) -> Result<VerificationJob, Error<E>> {
        let url = self.generate_url(&format!("v2/verify/{verification_id}"));
        for _ in 0..self.max_poll_attempts {
            let response = self
                .reqwest_client
                .get(url.clone())
                .send()
                .await
                .map_err(map_middleware_error)?;
            let job: VerificationJob = process_v2_response(response).await?;
            if job.is_job_completed {
                return Ok(job);
            }
            tokio::time::sleep(self.poll_interval).await;
        }

        Err(Error::Sourcify(SourcifyError::InternalServerError(
            format!(
                "verification job '{verification_id}' did not complete within {} attempts",
                self.max_poll_attempts
            ),
        )))
    }

    /// Resolves a completed job into either a verified contract (fetched from the
    /// contract endpoint) or an appropriately mapped error.
    async fn finalize_job<E: CustomError>(
        &self,
        chain_id: &str,
        contract_address: Bytes,
        job: VerificationJob,
    ) -> Result<VerifiedContract, Error<E>> {
        let is_matched = job
            .contract
            .as_ref()
            .is_some_and(|contract| contract.match_type.is_some());
        if is_matched {
            return self.get_contract_v2(chain_id, contract_address).await;
        }

        if let Some(error) = job.error {
            return Err(map_v2_error::<E>(error.custom_code, error.message, None));
        }

        Err(Error::Sourcify(SourcifyError::InternalServerError(
            "verification job completed without a contract match or an error".to_string(),
        )))
    }

    /// Fetches a verified contract (`GET /v2/contract/{chain}/{address}`),
    /// requesting only the fields required to reconstruct a verification success.
    async fn get_contract_v2<E: CustomError>(
        &self,
        chain_id: &str,
        contract_address: Bytes,
    ) -> Result<VerifiedContract, Error<E>> {
        let url = self.generate_url(&format!(
            "v2/contract/{chain_id}/{}",
            ToHex::to_hex(&contract_address)
        ));
        let response = self
            .reqwest_client
            .get(url)
            // Note: `match`/`runtimeMatch` are always-present base fields and are
            // rejected if passed as explicit field selectors.
            .query(&[("fields", "sources,metadata,creationBytecode")])
            .send()
            .await
            .map_err(map_middleware_error)?;

        let contract: ContractResponse = process_v2_response(response).await?;
        VerifiedContract::from_response(chain_id.to_string(), contract_address, contract)
            .map_err(|err| Error::Sourcify(SourcifyError::InternalServerError(err)))
    }
}

#[derive(Debug, Deserialize)]
struct SubmitResponse {
    #[serde(rename = "verificationId")]
    verification_id: String,
}

/// The `GenericErrorResponse` shape shared by all v2 error responses.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct V2ErrorResponse {
    custom_code: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    error_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractResponse {
    #[serde(rename = "match", default)]
    match_type: Option<String>,
    #[serde(default)]
    runtime_match: Option<String>,
    #[serde(default)]
    sources: Option<BTreeMap<String, SourceContent>>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    creation_bytecode: Option<CreationBytecode>,
}

#[derive(Debug, Deserialize)]
struct SourceContent {
    content: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreationBytecode {
    #[serde(default)]
    transformation_values: Option<TransformationValues>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TransformationValues {
    #[serde(default)]
    constructor_arguments: Option<String>,
}

impl VerifiedContract {
    fn from_response(
        chain_id: String,
        address: Bytes,
        response: ContractResponse,
    ) -> Result<Self, String> {
        let match_type = response
            .match_type
            .or(response.runtime_match)
            .ok_or_else(|| "sourcify contract response is missing a match status".to_string())
            .and_then(|value| {
                MatchType::from_v2(&value).ok_or_else(|| format!("unknown match status: {value}"))
            })?;

        let metadata = response
            .metadata
            .ok_or_else(|| "sourcify contract response is missing metadata".to_string())?;

        let sources = response
            .sources
            .unwrap_or_default()
            .into_iter()
            .map(|(path, source)| (path, source.content))
            .collect();

        let constructor_arguments = response
            .creation_bytecode
            .and_then(|bytecode| bytecode.transformation_values)
            .and_then(|values| values.constructor_arguments)
            .map(|hex| decode_hex(&hex).map(Bytes::from))
            .transpose()
            .map_err(|err| format!("invalid constructor arguments returned by sourcify: {err}"))?;

        Ok(Self {
            chain_id,
            address,
            match_type,
            sources,
            metadata,
            constructor_arguments,
        })
    }
}

impl MatchType {
    /// Parses a Sourcify API v2 match status (`exact_match` / `match`).
    pub(crate) fn from_v2(value: &str) -> Option<Self> {
        match value {
            "exact_match" => Some(MatchType::Full),
            "match" => Some(MatchType::Partial),
            _ => None,
        }
    }
}

fn map_middleware_error<E: CustomError>(error: reqwest_middleware::Error) -> Error<E> {
    match error {
        reqwest_middleware::Error::Middleware(err) => Error::ReqwestMiddleware(err),
        reqwest_middleware::Error::Reqwest(err) => Error::Reqwest(err),
    }
}

/// Deserializes a successful (`2xx`) v2 response, or maps an error response
/// (`GenericErrorResponse`) onto the appropriate [`Error`], giving the flow's
/// custom error type first crack at interpreting the `customCode`.
async fn process_v2_response<T: DeserializeOwned, E: CustomError>(
    response: Response,
) -> Result<T, Error<E>> {
    let status = response.status();
    if status.is_success() {
        return Ok(response.json::<T>().await?);
    }

    let error = response.json::<V2ErrorResponse>().await?;
    Err(map_v2_error::<E>(
        error.custom_code,
        error.message,
        Some(status),
    ))
}

/// Maps a v2 `customCode` (with optional HTTP status for the fallback) onto an
/// [`Error`]. The concrete custom error type `E` is consulted first, so that
/// endpoint-specific meanings (e.g. Etherscan-import failures) take precedence
/// over the generic interpretation.
fn map_v2_error<E: CustomError>(
    custom_code: String,
    message: Option<String>,
    status: Option<StatusCode>,
) -> Error<E> {
    let message = message.unwrap_or_default();

    if let Some(custom) = E::handle_custom_code(&custom_code, &message) {
        return Error::Sourcify(SourcifyError::Custom(custom));
    }

    let error = match custom_code.as_str() {
        "unsupported_chain" => SourcifyError::ChainNotSupported(message),
        "job_not_found" | "not_found" => SourcifyError::NotFound(message),
        // Terminal verification outcomes reported by a completed job.
        "no_match" | "compiler_error" | "verified_with_errors" => {
            SourcifyError::VerificationFailure(message)
        }
        "too_many_requests" | "etherscan_limit" => SourcifyError::UnexpectedStatusCode {
            status_code: StatusCode::TOO_MANY_REQUESTS,
            msg: message,
        },
        "internal_error" => SourcifyError::InternalServerError(message),
        _ => match status {
            Some(StatusCode::NOT_FOUND) => SourcifyError::NotFound(message),
            Some(StatusCode::BAD_REQUEST) => SourcifyError::BadRequest(message),
            Some(StatusCode::BAD_GATEWAY) => SourcifyError::BadGateway(message),
            Some(StatusCode::INTERNAL_SERVER_ERROR) => SourcifyError::InternalServerError(message),
            Some(status_code) => SourcifyError::UnexpectedStatusCode {
                status_code,
                msg: message,
            },
            None => SourcifyError::InternalServerError(message),
        },
    };
    Error::Sourcify(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ClientBuilder;
    use blockscout_display_bytes::decode_hex;

    fn client() -> Client {
        ClientBuilder::default()
            .try_base_url("https://staging.sourcify.dev/server/")
            .unwrap()
            .build()
    }

    // The exact inputs of the smart-contract-verifier `chain_not_supported_fail`
    // test: chain 2221 is not supported for Etherscan import, and v2 reports it
    // via a `400 unsupported_chain`, which must surface as a verification-level
    // `ChainNotSupported` (so downstream it becomes a FAILURE, not a 500).
    #[tokio::test]
    async fn verify_from_etherscan_v2_chain_not_supported() {
        let address = decode_hex("0xcb566e3B6934Fa77258d68ea18E931fa75e1aaAa")
            .unwrap()
            .into();
        let result = client()
            .verify_from_etherscan_v2("2221", address, None)
            .await
            .expect_err("error expected");
        assert!(
            matches!(
                &result,
                Error::Sourcify(SourcifyError::Custom(
                    VerifyFromEtherscanError::ChainNotSupported(message)
                )) if message.contains("is not supported for importing from Etherscan")
            ),
            "expected Custom(ChainNotSupported) with etherscan message, got: {result:?}"
        );
    }
}
