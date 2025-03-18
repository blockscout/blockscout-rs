use crate::repository::factory::FactoryDB;
use alloy::primitives::Address;

#[derive(Clone, Debug, PartialEq)]
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

impl From<Factory> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Factory {
    fn from(v: Factory) -> Self {
        Self {
            address: v.factory.to_string(),
            total_accounts: v.total_accounts,
        }
    }
}
