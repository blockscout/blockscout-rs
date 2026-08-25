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

        // Some Avalanche subnet RPC gateways (confirmed on Glacier's
        // `ext/bc/{chainId}/rpc` proxy, e.g. for chain 8021/Numine) have been
        // observed to return a *different* eth_call response than the one
        // requested for the same `to` address in quick succession — this
        // happens with sequential calls, fresh connections, and HTTP/1.1
        // alike, so it is not something a client-side ordering/pooling change
        // can prevent. Concretely: `decimals()`'s response can turn out to be
        // the raw bytes of a `name()`/`symbol()` call instead.
        //
        // Inspect the raw response before decoding rather than trust the
        // decoded `u8`: a genuine `decimals()` return is ABI-encoded as
        // exactly one right-padded 32-byte word, no matter its value,
        // while a dynamic `string` return (what `name()`/`symbol()` produce)
        // is always longer — offset word + length word + data. That
        // distinguishes a real `decimals() == 32` token (still exactly 32
        // raw bytes, accepted below) from this contamination (a much longer
        // response whose *first* word happens to decode as `32`, being the
        // ABI offset of the dynamic value it actually carries) — checking
        // the decoded value alone cannot tell those apart.
        let decimals_call = token_contract.decimals();
        let decimals_raw = decimals_call.call_raw().await?;
        anyhow::ensure!(
            is_valid_uint8_word(&decimals_raw),
            "decimals() returned a {}-byte response, not the single 32-byte \
             word a uint8 return is ABI-encoded as — likely the RPC gateway \
             returned a name()/symbol() response instead of decimals(); \
             rejecting as a failed fetch rather than persisting a misdecoded \
             value",
            decimals_raw.len()
        );
        let decimals = decimals_call.decode_output(decimals_raw)?;

        Ok(OnchainTokenInfo {
            name,
            symbol,
            decimals,
        })
    }
}

/// A `uint8` return value is always ABI-encoded as exactly one right-padded
/// 32-byte word, regardless of its value. Anything else — e.g. a dynamic
/// `string` return's offset+length+data encoding — cannot be a genuine
/// `decimals()` response.
fn is_valid_uint8_word(raw: &[u8]) -> bool {
    raw.len() == 32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_uint8_word_accepts_exactly_32_bytes() {
        assert!(is_valid_uint8_word(&[0u8; 32]));
    }

    #[test]
    fn test_is_valid_uint8_word_rejects_dynamic_string_encoding() {
        // offset word (0x20) + length word (0xc) + "Wrapped AVAX" padded to a
        // word: the actual raw shape observed from the misbehaving gateway.
        let mut raw = vec![0u8; 32];
        raw[31] = 0x20;
        raw.extend_from_slice(&[0u8; 32]);
        raw.extend_from_slice(b"Wrapped AVAX");
        raw.extend_from_slice(&[0u8; 20]);
        assert!(!is_valid_uint8_word(&raw));
    }

    #[test]
    fn test_is_valid_uint8_word_rejects_short_or_empty_response() {
        assert!(!is_valid_uint8_word(&[]));
        assert!(!is_valid_uint8_word(&[0u8; 31]));
    }
}
