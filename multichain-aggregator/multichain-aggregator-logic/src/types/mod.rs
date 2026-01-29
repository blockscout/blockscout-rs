pub mod address_coin_balances;
pub mod address_token_balances;
pub mod addresses;
pub mod api_keys;
pub mod batch_import_request;
pub mod block_ranges;
pub mod chain_metrics;
pub mod chains;
pub mod counters;
pub mod dapp;
pub mod domains;
pub mod hashes;
pub mod interop_message_transfers;
pub mod interop_messages;
pub mod portfolio;
pub mod sea_orm_wrappers;
pub mod search_results;
pub mod tokens;

pub type ChainId = i64;

pub fn proto_address_hash_from_alloy(
    address: &alloy_primitives::Address,
) -> crate::proto::AddressHash {
    crate::proto::AddressHash {
        hash: address.to_checksum(None),
    }
}

pub mod macros {
    macro_rules! opt_parse {
        ($v:expr) => {
            $v.map(|v| v.parse()).transpose()?
        };
    }

    pub(crate) use opt_parse;
}
