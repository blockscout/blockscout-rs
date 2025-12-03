use alloy::{network::Ethereum, providers::DynProvider};
use async_trait::async_trait;

pub struct OnchainTokenInfo {
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}
#[async_trait]
pub trait TokenInfoFetcher: Send + Sync {
    async fn fetch_token_info(
        &self,
        provider: &DynProvider<Ethereum>,
        chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<OnchainTokenInfo>;
}