use std::time::Duration;

use crate::InterchainDatabase;
use anyhow::{Context, Result, anyhow};
use backon::{ExponentialBuilder, Retryable};
use moka::future::Cache;
use reqwest::{StatusCode, Url, header};
use thiserror::Error;

const DATA_API_BASE_URL: &str = "https://data-api.avax.network";

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
}

impl TryFrom<&str> for AvalancheDataApiNetwork {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self> {
        match value {
            "mainnet" => Ok(Self::Mainnet),
            "fuji" => Ok(Self::Fuji),
            "testnet" => Ok(Self::Testnet),
            other => Err(anyhow!("unknown network: {}", other)),
        }
    }
}

/// Cache key: (network, blockchain_id_hex)
type CacheKey = [u8; 32];
type CacheValue = i64;

#[derive(Clone)]
pub struct BlockchainIdResolver {
    client: reqwest::Client,
    network: AvalancheDataApiNetwork,
    api_key: Option<String>,
    /// In-memory cache. `get_with` ensures only one inflight request per key.
    cache: Cache<CacheKey, CacheValue>,
    db: InterchainDatabase,
}

#[derive(Debug, serde::Deserialize)]
struct GetBlockchainByIdResponse {
    #[serde(rename = "blockchainId")]
    blockchain_id: String,
    #[serde(rename = "blockchainName")]
    blockchain_name: String,
    #[serde(rename = "evmChainId")]
    evm_chain_id: Option<i64>,
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

impl BlockchainIdResolver {
    pub fn new(
        network: AvalancheDataApiNetwork,
        api_key: Option<String>,
        db: InterchainDatabase,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            network,
            api_key,
            cache: Cache::new(10_000),
            db,
        }
    }

    pub async fn resolve(&self, blockchain_id: &[u8]) -> Result<i64> {
        let key: CacheKey = blockchain_id.try_into().map_err(|_| {
            anyhow!(
                "expected 32-byte blockchain_id, got {}",
                blockchain_id.len()
            )
        })?;

        let this = self.clone();
        self.cache
            .try_get_with(key, async move {
                if let Some(chain_id) = this
                    .db
                    .get_avalanche_icm_chain_id_by_blockchain_id(&key)
                    .await
                    .context("failed to query avalanche_icm_blockchain_ids")?
                {
                    return Ok::<CacheValue, anyhow::Error>(chain_id);
                }

                let resp = this
                    .fetch_with_backoff(&key)
                    .await
                    .context("failed to fetch blockchain info from Avalanche Data API")?;

                let chain_id = resp.evm_chain_id.context("missing evm_chain_id")?;
                let chain_name = resp.blockchain_name.clone();

                // Ensure FK target exists.
                if let Err(err) = this
                    .db
                    .ensure_chain_exists(chain_id, Some(chain_name), None)
                    .await
                {
                    tracing::warn!(
                        err = ?err,
                        chain_id,
                        blockchain_id = %resp.blockchain_id,
                        blockchain_name = ?resp.blockchain_name,
                        "failed to ensure chains row for discovered evmChainId"
                    );
                }

                if let Err(err) = this
                    .db
                    .upsert_avalanche_icm_blockchain_id(key.to_vec(), chain_id)
                    .await
                {
                    tracing::warn!(
                        err = ?err,
                        chain_id,
                        blockchain_id = %resp.blockchain_id,
                        blockchain_name = ?resp.blockchain_name,
                        "failed to upsert avalanche_icm_blockchain_ids row"
                    );
                }

                Ok::<CacheValue, anyhow::Error>(chain_id)
            })
            .await
            .map_err(|err| anyhow!(err.to_string()))
    }

    fn url(&self, blockchain_id: &[u8]) -> Result<Url> {
        let blockchain_id_cb58 = bs58::encode(blockchain_id).as_cb58(None).into_string();
        let url = format!(
            "{DATA_API_BASE_URL}/v1/networks/{}/blockchains/{}",
            self.network.as_str(),
            blockchain_id_cb58
        );
        Url::parse(&url).with_context(|| format!("failed to parse URL {url}"))
    }

    async fn fetch_with_backoff(&self, blockchain_id: &[u8]) -> Result<GetBlockchainByIdResponse> {
        let url = self.url(blockchain_id)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils;

    /// End-to-end test for the resolver.
    ///
    /// - Boots a real Postgres test DB (migrations applied)
    /// - Instantiates `BlockchainIdResolver`
    /// - Makes a real call to Avalanche Data API
    /// - Asserts the mapping is persisted
    ///
    /// Intentionally `#[ignore]` because it requires network access and can be
    /// flaky/rate-limited.
    #[tokio::test]
    #[ignore = "requires network access to data-api.avax.network and a postgres test db"]
    async fn resolves_native_id_to_chain_id_8021_and_persists_mapping() -> Result<()> {
        let native_id = "0xd32cc4660bcf8fa7971589f666fddb5ab22aee7e75dcb30b19829a65d4fb0063";

        let bytes = alloy::hex::decode(native_id.trim_start_matches("0x"))
            .context("native_id must be hex")?;
        anyhow::ensure!(bytes.len() == 32, "blockchainID must be 32 bytes");

        let db_guard = test_utils::init_db("resolver_resolves_8021").await;
        let db = db_guard.client();
        let interchain_db = InterchainDatabase::new(db.clone());

        // Optional API key support.
        let api_key = std::env::var("AVALANCHE_GLACIER_API_KEY")
            .ok()
            .or_else(|| std::env::var("AVALANCHE_DATA_API_KEY").ok())
            .filter(|s| !s.trim().is_empty());

        let resolver = BlockchainIdResolver::new(
            AvalancheDataApiNetwork::Mainnet,
            api_key,
            interchain_db.clone(),
        );

        let resolved = resolver.resolve(&bytes).await?;
        anyhow::ensure!(resolved == 8021, "expected 8021, got {:?}", resolved);

        let persisted = interchain_db
            .get_avalanche_icm_chain_id_by_blockchain_id(&bytes)
            .await?;
        anyhow::ensure!(
            persisted == Some(8021),
            "expected persisted 8021, got {:?}",
            persisted
        );

        Ok(())
    }
}
