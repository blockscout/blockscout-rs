use alloy::{network::Ethereum, primitives::Address, providers::DynProvider, sol};
use async_trait::async_trait;

use crate::token_info::fetchers::{Erc20TokenInfoFetcher, OnchainTokenInfo, TokenInfoFetcher};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ERC20TokenHome,
    "src/token_info/fetchers/abi/erc20_token_home.json"
);
pub struct Erc20TokenHomeInfoFetcher {
    erc20_fetcher: Erc20TokenInfoFetcher,
}

impl Erc20TokenHomeInfoFetcher {
    pub fn new() -> Self {
        Self { erc20_fetcher: Erc20TokenInfoFetcher }
    }
}

#[async_trait]
impl TokenInfoFetcher for Erc20TokenHomeInfoFetcher {
    async fn fetch_token_info(
        &self,
        provider: &DynProvider<Ethereum>,
        _chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<OnchainTokenInfo> {
        let token_contract = ERC20TokenHome::new(
            Address::from_slice(&address),
            provider.clone()
        );

        match token_contract.getTokenAddress().call().await {
            Ok(token_address) => {
                let token_info = self.erc20_fetcher.fetch_token_info(provider, _chain_id, token_address.to_vec()).await?;
                Ok(token_info)
            },
            Err(e) => {
                Err(e.into())
            }
        }
    }
}