use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use backon::{ExponentialBuilder, Retryable};
use reqwest::{StatusCode, Url, header};
use thiserror::Error;

pub const DATA_API_BASE_URL: &str = "https://data-api.avax.network";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Default)]
pub enum AvalancheDataApiNetwork {
    #[default]
    Mainnet,
    Fuji,
    Testnet,
}

impl AvalancheDataApiNetwork {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Fuji => "fuji",
            Self::Testnet => "testnet",
        }
    }

    pub fn from_env_or_default() -> Self {
        std::env::var("AVALANCHE_DATA_API_NETWORK")
            .ok()
            .and_then(|v| Self::try_from(v.as_str()).ok())
            .unwrap_or(Self::Mainnet)
    }
}

impl TryFrom<&str> for AvalancheDataApiNetwork {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "mainnet" => Ok(Self::Mainnet),
            "fuji" => Ok(Self::Fuji),
            "testnet" => Ok(Self::Testnet),
            other => Err(anyhow!("unknown network: {}", other)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AvalancheDataApiClient {
    client: reqwest::Client,
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

#[derive(Debug, Error)]
#[error("{inner}")]
struct RetryError {
    #[source]
    inner: anyhow::Error,
    is_retryable: bool,
}

impl RetryError {
    fn retryable(inner: impl Into<anyhow::Error>) -> Self {
        Self {
            inner: inner.into(),
            is_retryable: true,
        }
    }

    fn permanent(inner: impl Into<anyhow::Error>) -> Self {
        Self {
            inner: inner.into(),
            is_retryable: false,
        }
    }

    fn into_inner(self) -> anyhow::Error {
        self.inner
    }
}

impl AvalancheDataApiClient {
    pub fn new(network: AvalancheDataApiNetwork, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            network,
            api_key,
        }
    }

    pub fn with_client(
        client: reqwest::Client,
        network: AvalancheDataApiNetwork,
        api_key: Option<String>,
    ) -> Self {
        Self {
            client,
            network,
            api_key,
        }
    }

    fn blockchain_url(&self, blockchain_id: &[u8; 32]) -> Result<Url> {
        let blockchain_id_cb58 = bs58::encode(blockchain_id).as_cb58(None).into_string();
        let url = format!(
            "{DATA_API_BASE_URL}/v1/networks/{}/blockchains/{}",
            self.network.as_str(),
            blockchain_id_cb58
        );
        Url::parse(&url).with_context(|| format!("failed to parse URL {url}"))
    }

    pub async fn get_blockchain_by_id(
        &self,
        blockchain_id: &[u8; 32],
    ) -> Result<GetBlockchainByIdResponse> {
        let url = self.blockchain_url(blockchain_id)?;
        self.fetch_with_backoff(url).await
    }

    async fn fetch_with_backoff(&self, url: Url) -> Result<GetBlockchainByIdResponse> {
        let fetch = || async {
            let req = self
                .client
                .get(url.as_str())
                .header(header::ACCEPT, "application/json");

            let req = if let Some(key) = self.api_key.as_deref() {
                req.header("x-glacier-api-key", key)
            } else {
                req
            };

            let resp = req.send().await.map_err(RetryError::retryable)?;

            match resp.status() {
                status if status.is_success() => resp
                    .json::<GetBlockchainByIdResponse>()
                    .await
                    .map_err(RetryError::retryable),

                StatusCode::TOO_MANY_REQUESTS => {
                    Err(RetryError::retryable(anyhow!("rate limited")))
                }

                status if status.is_server_error() => {
                    Err(RetryError::retryable(anyhow!("server error: {status}")))
                }

                status => {
                    let body = resp.text().await.unwrap_or_default();
                    Err(RetryError::permanent(anyhow!(
                        "unexpected response: {} - {}",
                        status,
                        body
                    )))
                }
            }
        };

        let backoff = ExponentialBuilder::default()
            .with_min_delay(Duration::from_millis(200))
            .with_max_delay(Duration::from_secs(5))
            .with_max_times(5);

        fetch
            .retry(backoff)
            .when(|e| e.is_retryable)
            .notify(|err, duration| {
                tracing::warn!(?url, ?err, ?duration, "retrying Avalanche Data API request");
            })
            .await
            .map_err(RetryError::into_inner)
            .context("Avalanche Data API request failed")
    }
}
