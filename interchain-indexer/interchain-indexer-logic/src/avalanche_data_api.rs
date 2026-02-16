use serde::{Deserialize, Serialize};
use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::{Url, header};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{RetryTransientMiddleware, policies::ExponentialBackoff};
use strum_macros::{AsRefStr, EnumString};

pub const DATA_API_BASE_URL: &str = "https://data-api.avax.network";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct AvalancheDataApiClientSettings {
    pub network: AvalancheDataApiNetwork,
    pub api_key: Option<String>,
}

#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Default, EnumString, AsRefStr, Serialize, Deserialize,
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

    fn blockchain_url(&self, blockchain_id: &[u8; 32]) -> Result<Url> {
        let blockchain_id_cb58 = bs58::encode(blockchain_id).as_cb58(None).into_string();
        let url = format!(
            "{DATA_API_BASE_URL}/v1/networks/{}/blockchains/{}",
            self.network.as_ref(),
            blockchain_id_cb58
        );
        Url::parse(&url).with_context(|| format!("failed to parse URL {url}"))
    }

    pub async fn get_blockchain_by_id(
        &self,
        blockchain_id: &[u8; 32],
    ) -> Result<GetBlockchainByIdResponse> {
        let url = self.blockchain_url(blockchain_id)?;

        let mut req = self
            .client
            .get(url.as_str())
            .header(header::ACCEPT, "application/json");

        if let Some(key) = self.api_key.as_deref() {
            req = req.header("x-glacier-api-key", key);
        }

        let response = req
            .send()
            .await
            .context("Avalanche Data API request failed")?
            .error_for_status()
            .context("Avalanche Data API returned non-success status")?
            .json::<GetBlockchainByIdResponse>()
            .await
            .context("failed to deserialize Avalanche Data API response")?;

        Ok(response)
    }
}
