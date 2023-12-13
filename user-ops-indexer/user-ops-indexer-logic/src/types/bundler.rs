use ethers::prelude::Address;
use ethers_core::utils::to_checksum;

pub use entity::sea_orm_active_enums::SponsorType;

use crate::repository::bundler::BundlerDB;

#[derive(Clone, Debug, PartialEq)]
pub struct Bundler {
    pub bundler: Address,
    pub total_bundles: u32,
    pub total_ops: u32,
}

impl From<BundlerDB> for Bundler {
    fn from(v: BundlerDB) -> Self {
        Self {
            bundler: Address::from_slice(&v.bundler),
            total_bundles: v.total_bundles as u32,
            total_ops: v.total_ops as u32,
        }
    }
}

impl From<Bundler> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Bundler {
    fn from(v: Bundler) -> Self {
        Self {
            address: to_checksum(&v.bundler, None),
            total_bundles: v.total_bundles,
            total_ops: v.total_ops,
        }
    }
}
