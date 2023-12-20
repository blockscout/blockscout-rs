use crate::repository::paymaster::PaymasterDB;
pub use entity::sea_orm_active_enums::SponsorType;
use ethers::prelude::Address;
use ethers_core::utils::to_checksum;

#[derive(Clone, Debug, PartialEq)]
pub struct Paymaster {
    pub paymaster: Address,
    pub total_ops: u32,
}

impl From<PaymasterDB> for Paymaster {
    fn from(v: PaymasterDB) -> Self {
        Self {
            paymaster: Address::from_slice(&v.paymaster),
            total_ops: v.total_ops as u32,
        }
    }
}

impl From<Paymaster> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Paymaster {
    fn from(v: Paymaster) -> Self {
        Self {
            address: to_checksum(&v.paymaster, None),
            total_ops: v.total_ops,
        }
    }
}
