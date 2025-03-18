use alloy::primitives::Address;
use bens_logic::subgraph::{Order, ResolverInSubgraph};
use bens_proto::blockscout::bens::v1 as proto;
use nonempty::NonEmpty;
use std::str::FromStr;
use thiserror::Error;

mod domain;
mod events;
mod protocol;

pub use domain::*;
pub use events::*;
pub use protocol::*;

const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("invalid argument: {0}")]
    UserRequest(String),
    #[error("internal error: {0}")]
    #[allow(dead_code)]
    LogicOutput(String),
}

#[inline]
pub fn order_direction_from_inner(inner: proto::Order) -> Order {
    match inner {
        proto::Order::Unspecified => Order::Desc,
        proto::Order::Asc => Order::Asc,
        proto::Order::Desc => Order::Desc,
    }
}

#[inline]
pub fn checksummed(address: &Address, chain_id: i64) -> String {
    match chain_id {
        30 | 31 => address.to_checksum(Some(chain_id as u64)),
        _ => address.to_checksum(None),
    }
}

#[inline]
pub fn address_from_logic(address: &Address, chain_id: i64) -> proto::Address {
    proto::Address {
        hash: checksummed(address, chain_id),
    }
}

#[inline]
pub fn address_from_str_logic(
    addr: &str,
    chain_id: i64,
) -> Result<proto::Address, ConversionError> {
    let addr = Address::from_str(addr)
        .map_err(|e| ConversionError::LogicOutput(format!("invalid address '{addr}': {e}")))?;
    Ok(address_from_logic(&addr, chain_id))
}

#[inline]
pub fn resolver_from_logic(
    resolver: String,
    chain_id: i64,
) -> Result<proto::Address, ConversionError> {
    let resolver = ResolverInSubgraph::from_str(&resolver)
        .map_err(|e| ConversionError::LogicOutput(e.to_string()))?;
    Ok(address_from_logic(&resolver.resolver_address, chain_id))
}

#[inline]
pub fn and_not_zero_address(address: proto::Address) -> Option<proto::Address> {
    if address.hash == ZERO_ADDRESS {
        None
    } else {
        Some(address)
    }
}

#[inline]
pub fn maybe_protocol_filter_from_inner(maybe_filter: Option<String>) -> Option<NonEmpty<String>> {
    maybe_filter.and_then(|f| NonEmpty::collect(f.split(',').map(|s| s.to_string())))
}
