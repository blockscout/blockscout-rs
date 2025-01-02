use crate::repository::bundle::BundleDB;
use alloy::primitives::{Address, TxHash};

#[derive(Clone, Debug, PartialEq)]
pub struct Bundle {
    pub transaction_hash: TxHash,
    pub bundle_index: u32,
    pub block_number: u64,
    pub bundler: Address,
    pub timestamp: String,
    pub total_ops: u32,
}

impl From<BundleDB> for Bundle {
    fn from(v: BundleDB) -> Self {
        Self {
            transaction_hash: TxHash::from_slice(&v.transaction_hash),
            bundle_index: v.bundle_index as u32,
            block_number: v.block_number as u64,
            bundler: Address::from_slice(&v.bundler),
            total_ops: v.total_ops as u32,
            timestamp: v
                .timestamp
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
        }
    }
}

impl From<Bundle> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Bundle {
    fn from(v: Bundle) -> Self {
        Self {
            transaction_hash: v.transaction_hash.to_string(),
            bundler: v.bundler.to_string(),
            block_number: v.block_number,
            bundle_index: v.bundle_index,
            timestamp: v.timestamp,
            total_ops: v.total_ops,
        }
    }
}
