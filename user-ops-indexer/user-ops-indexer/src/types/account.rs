use ethers::prelude::{Address, H256};
use ethers_core::abi::AbiEncode;
use ethers_core::utils::to_checksum;

pub use entity::sea_orm_active_enums::SponsorType;

use crate::repository::account::AccountDB;

#[derive(Clone)]
pub struct Account {
    pub address: Address,
    pub factory: Option<Address>,
    pub creation_tx_hash: Option<H256>,
    pub creation_op_hash: Option<H256>,
    pub creation_timestamp: Option<u32>,
    pub total_ops: u32,
}

impl From<AccountDB> for Account {
    fn from(v: AccountDB) -> Self {
        Self {
            address: Address::from_slice(&v.address),
            factory: v.factory.map(|a| Address::from_slice(&a)),
            creation_tx_hash: v.creation_tx_hash.map(|a| H256::from_slice(&a)),
            creation_op_hash: v.creation_op_hash.map(|a| H256::from_slice(&a)),
            creation_timestamp: v.creation_timestamp.map(|t| t.timestamp() as u32),
            total_ops: v.total_ops as u32,
        }
    }
}

impl Into<user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Account> for Account {
    fn into(self) -> user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Account {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Account {
            address: to_checksum(&self.address, None),
            factory: self.factory.map(|a| to_checksum(&a, None)),
            creation_tx_hash: self.creation_tx_hash.map(|a| a.encode_hex()),
            creation_op_hash: self.creation_op_hash.map(|a| a.encode_hex()),
            creation_timestamp: self.creation_timestamp,
            total_ops: self.total_ops,
        }
    }
}
