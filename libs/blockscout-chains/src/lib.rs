use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::Deserialize;
use std::collections::HashMap;

const CHAINS_URL: &str = "https://chains.blockscout.com/api/chains";

pub async fn get_blockscout_chains() -> anyhow::Result<BlockscoutChains> {
    let max_retries = 3;
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retries);
    let client = ClientBuilder::new(reqwest::Client::new())
        .with(RetryTransientMiddleware::new_with_policy(retry_policy))
        .build();
    let res = client.get(CHAINS_URL).send().await?;
    let chains: BlockscoutChains = res.json().await?;
    Ok(chains)
}

pub type BlockscoutChains = HashMap<i64, BlockscoutChainData>;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlockscoutChainData {
    pub name: String,
    pub description: String,
    pub ecosystem: Ecosystem,
    pub is_testnet: Option<bool>,
    pub layer: Option<u8>,
    pub rollup_type: Option<String>,
    pub website: String,
    pub explorers: Vec<ExplorerConfig>,
    pub logo: String,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Ecosystem {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ExplorerConfig {
    pub url: String,
    pub hosted_by: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_blockscout_chains() {
        let chains = get_blockscout_chains().await.unwrap();
        assert!(!chains.is_empty());
    }
}
