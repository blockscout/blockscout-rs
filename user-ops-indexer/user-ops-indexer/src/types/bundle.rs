use ethers::prelude::{Address, H256};
use ethers_core::abi::AbiEncode;
use ethers_core::utils::to_checksum;

pub use entity::sea_orm_active_enums::SponsorType;

use crate::repository::bundle::BundleDB;

#[derive(Clone)]
pub struct Bundle {
    pub tx_hash: H256,
    pub bundle_index: u64,
    pub block_number: u64,
    pub bundler: Address,
    pub timestamp: u32,
    pub total_ops: u32,
}

impl From<BundleDB> for Bundle {
    fn from(v: BundleDB) -> Self {
        Self {
            tx_hash: H256::from_slice(&v.tx_hash),
            bundle_index: v.bundle_index as u64,
            block_number: v.block_number as u64,
            bundler: Address::from_slice(&v.bundler),
            total_ops: v.total_ops as u32,
            timestamp: v.timestamp.timestamp() as u32,
        }
    }
}

impl Into<user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Bundle> for Bundle {
    fn into(self) -> user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Bundle {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Bundle {
            tx_hash: self.tx_hash.encode_hex(),
            bundler: to_checksum(&self.bundler, None),
            block_number: self.block_number,
            bundle_index: self.bundle_index,
            timestamp: self.timestamp,
            total_ops: self.total_ops,
        }
    }
}
