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
    type Error = ();

    async fn set(&self, key: SmartContractId, value: SmartContractValue) -> Result<(), ()> {
        contract_url::Entity::insert(contract_url::ActiveModel {
            chain_id: Set(key.chain_id),
            address: Set(key.address.to_string()),
            url: Set(value.blockscout_url.to_string()),
        })
        .exec(&self.db)
        .await
        .unwrap();
        Ok(())
    }

    async fn replace(
        &self,
        key: SmartContractId,
        value: SmartContractValue,
    ) -> Result<Option<SmartContractValue>, ()> {
        let existing_value = self.get(&key).await?;
        self.set(key, value).await?;
        Ok(existing_value)
    }

    async fn get(&self, key: &SmartContractId) -> Result<Option<SmartContractValue>, ()> {
        let find_key = (key.chain_id.clone(), key.address.to_string());
        let find_result = contract_url::Entity::find_by_id(find_key)
            .one(&self.db)
            .await
            .unwrap();
        Ok(find_result.map(|a| SmartContractValue {
            blockscout_url: url::Url::parse(&a.url).unwrap(),
            sources: BTreeMap::new(),
        }))
    }

    async fn remove(&self, key: &SmartContractId) -> Result<Option<SmartContractValue>, ()> {
        todo!()
    }
}
