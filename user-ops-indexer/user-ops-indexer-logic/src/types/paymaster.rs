use crate::repository::paymaster::PaymasterDB;
use alloy::primitives::Address;

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
            address: v.paymaster.to_string(),
            total_ops: v.total_ops,
        }
    }
}
