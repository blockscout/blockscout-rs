use bens_logic::subgraphs_reader::Order;
use bens_proto::blockscout::bens::v1 as proto;
use ethers::{addressbook::Address, utils::to_checksum};
use std::str::FromStr;
use thiserror::Error;

mod domain;
mod events;

pub use domain::*;
pub use events::*;

pub fn order_direction_from_inner(inner: proto::Order) -> Order {
    match inner {
        proto::Order::Unspecified => Order::Desc,
        proto::Order::Asc => Order::Asc,
        proto::Order::Desc => Order::Desc,
    }
}

#[derive(Error, Debug)]
pub enum ConversionError {
    #[error("invalid argument: {0}")]
    UserRequest(String),
    #[error("internal error: {0}")]
    #[allow(dead_code)]
    LogicOutput(String),
}

fn checksummed(address: &Address, chain_id: i64) -> String {
    match chain_id {
        30 | 31 => to_checksum(address, Some(chain_id as u8)),
        _ => to_checksum(address, None),
    }
}

fn address_from_logic(address: Address, chain_id: i64) -> proto::Address {
    proto::Address {
        hash: checksummed(&address, chain_id),
    }
}

fn address_from_str_logic(addr: &str, chain_id: i64) -> Result<proto::Address, ConversionError> {
    let addr = Address::from_str(addr)
        .map_err(|e| ConversionError::LogicOutput(format!("invalid address '{addr}': {e}")))?;
    Ok(proto::Address {
        hash: checksummed(&addr, chain_id),
    })
}
