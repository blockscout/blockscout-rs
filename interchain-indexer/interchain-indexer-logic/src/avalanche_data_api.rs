// SPDX-License-Identifier: LicenseRef-Blockscout

use serde::{Deserialize, Serialize};
use std::time::Duration;

use reqwest::{Url, header};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use strum_macros::{AsRefStr, EnumString};

pub const DATA_API_BASE_URL: &str = "https://data-api.avax.network";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct AvalancheDataApiClientSettings {
    pub network: AvalancheDataApiNetwork,
    pub api_key: Option<String>,
}

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Default,
    EnumString,
    AsRefStr,
    strum_macros::Display,
    Serialize,
    Deserialize,
)]
#[strum(serialize_all = "lowercase", ascii_case_insensitive)]
pub enum AvalancheDataApiNetwork {
    #[default]
    Mainnet,
    Fuji,
    Testnet,
}

#[derive(Clone, Debug)]
pub struct AvalancheDataApiClient {
    client: ClientWithMiddleware,
    network: AvalancheDataApiNetwork,
    api_key: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct GetBlockchainByIdResponse {
    #[serde(rename = "blockchainId")]
    pub blockchain_id: String,
    #[serde(rename = "blockchainName")]
    pub blockchain_name: String,
    #[serde(rename = "evmChainId")]
    pub evm_chain_id: Option<i64>,
}

impl AvalancheDataApiClient {
    pub fn new(network: AvalancheDataApiNetwork, api_key: Option<String>) -> Self {
        let base_client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .expect("failed to build reqwest client");

        let retry_policy = ExponentialBackoff::builder()
            .retry_bounds(Duration::from_millis(200), Duration::from_secs(5))
            .build_with_max_retries(5);

        let client = ClientBuilder::new(base_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Self {
            client,
            network,
            api_key,
        }
    }

    pub fn from_settings(settings: AvalancheDataApiClientSettings) -> Self {
        Self::new(settings.network, settings.api_key)
    }

    fn blockchain_url(&self, blockchain_id: &[u8; 32]) -> Result<Url, DataApiError> {
        let blockchain_id_cb58 = blockchain_id_to_cb58(blockchain_id);
        let url = format!(
            "{DATA_API_BASE_URL}/v1/networks/{}/blockchains/{}",
            self.network.as_ref(),
            blockchain_id_cb58
        );
        Url::parse(&url).map_err(DataApiError::Url)
    }

    pub async fn get_blockchain_by_id(
        &self,
        blockchain_id: &[u8; 32],
    ) -> Result<GetBlockchainByIdResponse, DataApiError> {
        let url = self.blockchain_url(blockchain_id)?;

        let mut req = self
            .client
            .get(url.as_str())
            .header(header::ACCEPT, "application/json");

        if let Some(key) = self.api_key.as_deref() {
            req = req.header("x-glacier-api-key", key);
        }

        let response = req.send().await.map_err(DataApiError::Transport)?;
        let status = response.status();
        if status.is_success() {
            response
                .json::<GetBlockchainByIdResponse>()
                .await
                .map_err(DataApiError::Decode)
        } else {
            // The body is needed for classification; its unavailability is
            // itself an "unrecognized" case, not a reason to lose the error.
            let body = response.text().await.ok();
            Err(classify_error_response(
                status,
                body.as_deref(),
                self.network,
            ))
        }
    }
}

/// Errors from calling the Avalanche Data API's blockchain-lookup endpoint.
#[derive(Debug, thiserror::Error)]
pub enum DataApiError {
    /// The Data API's network does not know this blockchain. The only
    /// variant callers may treat as data rather than a processing failure.
    #[error("blockchain not found in {network} network")]
    BlockchainNotFound { network: AvalancheDataApiNetwork },
    #[error("Avalanche Data API returned status {status}: {message}")]
    Status {
        status: reqwest::StatusCode,
        message: String,
    },
    #[error("Avalanche Data API request failed")]
    Transport(#[source] reqwest_middleware::Error),
    #[error("failed to deserialize Avalanche Data API response")]
    Decode(#[source] reqwest::Error),
    #[error("failed to build Avalanche Data API URL")]
    Url(#[source] url::ParseError),
}

/// The exact `message` text the Data API responds with for a blockchain ID
/// it does not recognize (observed live 2026-09-10). This is an external
/// API's observed contract, not an internal convention — do not weaken the
/// comparison to `contains("not found")`: the *other* 404 shape ("Cannot GET
/// /v1/networks/.../blockchains/...") also carries `"error":"Not Found"` in
/// the same body and must NOT be classified as `BlockchainNotFound`.
const BLOCKCHAIN_NOT_FOUND_MESSAGE: &str = "Blockchain not found";

/// Tolerant read of the Data API's JSON error envelope. `message` is a
/// string for a not-found/generic error, but an array of strings for
/// validation errors (e.g. malformed blockchain ID) — read it as `Value` and
/// only match the string form, so any shape decodes without error.
#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    message: serde_json::Value,
}

/// Classifies a non-success Data API response by body content, not by
/// status/content-type alone: a 404 for an unknown (but validly-shaped)
/// blockchain ID and a 404 for a mistyped API path are otherwise
/// indistinguishable (same status, same `Content-Type`, same top-level
/// `"error":"Not Found"`) — only `message` differs. Anything that isn't
/// exactly the known not-found message — including an unparseable body —
/// stays `Status` and remains retryable.
fn classify_error_response(
    status: reqwest::StatusCode,
    body: Option<&str>,
    network: AvalancheDataApiNetwork,
) -> DataApiError {
    let is_known_not_found = status == reqwest::StatusCode::NOT_FOUND
        && body
            .and_then(|body| serde_json::from_str::<ErrorEnvelope>(body).ok())
            .and_then(|envelope| envelope.message.as_str().map(str::trim).map(str::to_string))
            .is_some_and(|message| message.eq_ignore_ascii_case(BLOCKCHAIN_NOT_FOUND_MESSAGE));

    if is_known_not_found {
        return DataApiError::BlockchainNotFound { network };
    }

    DataApiError::Status {
        status,
        message: body.unwrap_or_default().to_string(),
    }
}

/// CB58 encoding of a blockchain ID, as used both in Data API request URLs
/// and rendered in `protocol_metadata` for display. Kept in one place so the
/// two call sites cannot drift into two different encodings.
pub fn blockchain_id_to_cb58(blockchain_id: &[u8; 32]) -> String {
    bs58::encode(blockchain_id).as_cb58(None).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    const NETWORK: AvalancheDataApiNetwork = AvalancheDataApiNetwork::Mainnet;

    #[test]
    fn classify_404_blockchain_not_found() {
        let body = r#"{"message":"Blockchain not found","error":"Not Found","statusCode":404}"#;
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, Some(body), NETWORK),
            DataApiError::BlockchainNotFound { .. }
        ));
    }

    #[test]
    fn classify_404_cannot_get_is_status_not_not_found() {
        let body = r#"{"message":"Cannot GET /v1/networks/mainnet/blockchains/222/yH8D7Th","error":"Not Found","statusCode":404}"#;
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, Some(body), NETWORK),
            DataApiError::Status {
                status: StatusCode::NOT_FOUND,
                ..
            }
        ));
    }

    #[test]
    fn classify_404_html_body_is_status() {
        let body = "<html><body>404 Not Found</body></html>";
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, Some(body), NETWORK),
            DataApiError::Status { .. }
        ));
    }

    #[test]
    fn classify_404_unreadable_body_is_status() {
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, None, NETWORK),
            DataApiError::Status { .. }
        ));
    }

    #[test]
    fn classify_404_array_message_is_status() {
        let body = r#"{"message":["blockchainId must be base58 encoded"],"error":"Bad Request","statusCode":400}"#;
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, Some(body), NETWORK),
            DataApiError::Status { .. }
        ));
    }

    #[test]
    fn classify_400_invalid_network_is_status() {
        let body = r#"{"message":"Invalid network type","error":"Bad Request","statusCode":400}"#;
        assert!(matches!(
            classify_error_response(StatusCode::BAD_REQUEST, Some(body), NETWORK),
            DataApiError::Status { .. }
        ));
    }

    #[test]
    fn classify_400_invalid_id_is_status() {
        let body = r#"{"message":["blockchainId must be base58 encoded"],"error":"Bad Request","statusCode":400}"#;
        assert!(matches!(
            classify_error_response(StatusCode::BAD_REQUEST, Some(body), NETWORK),
            DataApiError::Status { .. }
        ));
    }

    #[test]
    fn classify_other_statuses_are_status() {
        for status in [
            StatusCode::UNAUTHORIZED,
            StatusCode::FORBIDDEN,
            StatusCode::TOO_MANY_REQUESTS,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            assert!(matches!(
                classify_error_response(status, None, NETWORK),
                DataApiError::Status { .. }
            ));
        }
    }

    #[test]
    fn classify_404_blockchain_not_found_trims_and_ignores_case() {
        let body = r#"{"message":"  blockchain not found  ","error":"Not Found","statusCode":404}"#;
        assert!(matches!(
            classify_error_response(StatusCode::NOT_FOUND, Some(body), NETWORK),
            DataApiError::BlockchainNotFound { .. }
        ));
    }
}
