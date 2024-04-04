use std::collections::BTreeMap;

use basic_cache_entity::{contract_sources, contract_url};
use sea_orm::{ActiveValue::Set, DatabaseConnection, EntityTrait};

use crate::{
    types::{SmartContractId, SmartContractValue},
    CacheManager,
};

pub struct PostgresCache {
    db: DatabaseConnection,
}

impl PostgresCache {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl CacheManager<SmartContractId, SmartContractValue> for PostgresCache {
    async fn insert(
        &self,
        key: SmartContractId,
        value: SmartContractValue,
    ) -> Option<SmartContractValue> {
        contract_url::Entity::insert(contract_url::ActiveModel {
            chain_id: Set(key.chain_id),
            address: Set(key.address.to_string()),
            url: Set(value.blockscout_url.to_string()),
        })
        .exec(&self.db)
        .await
        .unwrap();
        None
    }

    async fn get(&self, key: &SmartContractId) -> Option<SmartContractValue> {
        let find_key = (key.chain_id.clone(), key.address.to_string());
        let find_result = contract_url::Entity::find_by_id(find_key)
            .one(&self.db)
            .await
            .unwrap();
        find_result.map(|a| SmartContractValue {
            blockscout_url: url::Url::parse(&a.url).unwrap(),
            sources: BTreeMap::new(),
        })
    }

    async fn remove(&self, key: &SmartContractId) -> Option<SmartContractValue> {
        todo!()
    }
}
