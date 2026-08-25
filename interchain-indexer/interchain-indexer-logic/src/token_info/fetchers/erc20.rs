// SPDX-License-Identifier: LicenseRef-Blockscout

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
        if address.len() != 20 {
            anyhow::bail!("ERC20 address must be 20 bytes, got {}", address.len());
        }

        let token_contract = ERC20::new(Address::from_slice(&address), provider.clone());

        let name = token_contract.name().call().await?;
        let symbol = token_contract.symbol().call().await?;
        let decimals = token_contract.decimals().call().await?;

        // Some Avalanche subnet RPC gateways (confirmed on Glacier's
        // `ext/bc/{chainId}/rpc` proxy, e.g. for chain 8021/Numine) have been
        // observed to return a *different* eth_call response than the one
        // requested for the same `to` address in quick succession — this
        // happens with sequential calls, fresh connections, and HTTP/1.1
        // alike, so it is not something a client-side ordering/pooling change
        // can prevent. When `decimals()`'s response is actually the payload
        // of a `name()`/`symbol()` call, the ABI-encoded uint8 is decoded
        // from that response's leading word, which for any single dynamic
        // return value (per the ABI spec) is always the offset 0x20 = 32.
        // That makes exactly 32 a near-unmistakable signature of this
        // contamination rather than a genuine decimals value: treat it as a
        // failed fetch instead of persisting corrupted decimals, and let the
        // existing "decimals IS NULL" background retry
        // (`kickoff_token_fetch_for_stats_enrichment`) pick it up again
        // later, once the gateway is no longer misbehaving for this address.
        anyhow::ensure!(
            !is_abi_offset_artifact(decimals),
            "decimals() returned 32 — the ABI offset word for a single \
             dynamic-type return value, indicating the RPC gateway likely \
             returned a name()/symbol() response instead of decimals(); \
             rejecting as a failed fetch rather than persisting it"
        );

        Ok(OnchainTokenInfo {
            name,
            symbol,
            decimals,
        })
    }
}

/// True for the one value a `decimals()` response cannot plausibly carry
/// unless it is actually a `name()`/`symbol()` response: per the ABI spec,
/// a single dynamic-type return value is always encoded with a leading
/// offset word of 0x20 (32), so decoding *that* as a `uint8` yields exactly
/// 32 regardless of the string's actual contents.
fn is_abi_offset_artifact(decimals: u8) -> bool {
    decimals == 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_abi_offset_artifact_flags_only_32() {
        assert!(is_abi_offset_artifact(32));
        assert!(!is_abi_offset_artifact(18));
        assert!(!is_abi_offset_artifact(6));
        assert!(!is_abi_offset_artifact(0));
        assert!(!is_abi_offset_artifact(255));
    }
}
