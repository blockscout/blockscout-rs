use ethers::prelude::Address;
use ethers_core::utils::to_checksum;

pub use entity::sea_orm_active_enums::SponsorType;

use crate::repository::factory::FactoryDB;

#[derive(Clone)]
pub struct Factory {
    pub factory: Address,
    pub total_accounts: u32,
}

impl From<FactoryDB> for Factory {
    fn from(v: FactoryDB) -> Self {
        Self {
            factory: Address::from_slice(&v.factory),
            total_accounts: v.total_accounts as u32,
        }
    }
}

impl Into<user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Factory> for Factory {
    fn into(self) -> user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Factory {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Factory {
            address: to_checksum(&self.factory, None),
            total_accounts: self.total_accounts,
        }
    }
}
