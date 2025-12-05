use alloy::{network::Ethereum, primitives::Address, providers::DynProvider, sol};
use async_trait::async_trait;

use crate::token_info::fetchers::{OnchainTokenInfo, TokenInfoFetcher};

sol!(
    #[allow(missing_docs)]
    #[sol(rpc)]
    ERC20,
    "src/token_info/fetchers/abi/erc20.json"
);
pub struct Erc20TokenInfoFetcher;

#[async_trait]
impl TokenInfoFetcher for Erc20TokenInfoFetcher {
    async fn fetch_token_info(
        &self,
        provider: &DynProvider<Ethereum>,
        _chain_id: u64,
        address: Vec<u8>,
    ) -> anyhow::Result<OnchainTokenInfo> {
        let token_contract = ERC20::new(Address::from_slice(&address), provider.clone());
        let name_call = token_contract.name();
        let symbol_call = token_contract.symbol();
        let decimals_call = token_contract.decimals();

        match tokio::try_join!(name_call.call(), symbol_call.call(), decimals_call.call()) {
            Ok((name, symbol, decimals)) => Ok(OnchainTokenInfo {
                name,
                symbol,
                decimals,
            }),
            Err(e) => Err(e.into()),
        }
    }
}
