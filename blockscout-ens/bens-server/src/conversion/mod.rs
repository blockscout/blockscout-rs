use bens_logic::subgraphs_reader::Order;
use bens_proto::blockscout::bens::v1 as proto;
use ethers::{addressbook::Address, utils::to_checksum};
use nonempty::NonEmpty;
use std::str::FromStr;
use thiserror::Error;

mod domain;
mod events;
mod protocol;

pub use domain::*;
pub use events::*;
pub use protocol::*;

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
        30 | 31 => to_checksum(address, Some(chain_id as u8)),
        _ => to_checksum(address, None),
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
pub fn maybe_protocol_filter_from_inner(maybe_filter: Option<String>) -> Option<NonEmpty<String>> {
    maybe_filter.and_then(|f| NonEmpty::collect(f.split(',').map(|s| s.to_string())))
}
