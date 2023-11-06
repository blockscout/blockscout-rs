use bens_logic::subgraphs_reader::Order;
use bens_proto::blockscout::bens::v1 as proto;
use thiserror::Error;

mod domain;
mod events;

pub use domain::*;
pub use events::*;

pub fn order_direction_from_inner(inner: proto::Order) -> Order {
    match inner {
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
