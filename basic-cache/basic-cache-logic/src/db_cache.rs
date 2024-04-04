use std::collections::BTreeMap;

use basic_cache_entity::{contract_sources, contract_url};

use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ConnectionTrait, DatabaseConnection, DbErr,
    EntityTrait, Iterable, Statement,
};

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

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Unsuccessful DB operation")]
    Database(#[from] DbErr),
    #[error("Data stored in DB is corrupted")]
    InvalidData(#[from] url::ParseError),
}

impl CacheManager<SmartContractId, SmartContractValue> for PostgresCache {
    type Error = Error;

    async fn set(
        &self,
        key: SmartContractId,
        value: SmartContractValue,
    ) -> Result<(), Self::Error> {
        contract_url::Entity::insert(contract_url::ActiveModel {
            chain_id: Set(key.chain_id.clone()),
            address: Set(key.address.to_string()),
            url: Set(value.blockscout_url.to_string()),
        })
        .on_conflict(
            OnConflict::columns(contract_url::PrimaryKey::iter())
                .update_column(contract_url::Column::Url)
                .to_owned(),
        )
        .exec(&self.db)
        .await?;

        let sources_models =
            value
                .sources
                .into_iter()
                .map(|(filename, contents)| contract_sources::ActiveModel {
                    chain_id: Set(key.chain_id.clone()),
                    address: Set(key.address.to_string()),
                    filename: Set(filename),
                    contents: Set(contents),
                });

        contract_sources::Entity::insert_many(sources_models)
            .on_conflict(
                OnConflict::columns(contract_sources::PrimaryKey::iter())
                    .update_column(contract_sources::Column::Contents)
                    .to_owned(),
            )
            .exec(&self.db)
            .await?;
        Ok(())
    }

    async fn replace(
        &self,
        key: SmartContractId,
        value: SmartContractValue,
    ) -> Result<Option<SmartContractValue>, Self::Error> {
        let existing_value = self.get(&key).await?;
        self.set(key, value).await?;
        Ok(existing_value)
    }

    async fn get(&self, key: &SmartContractId) -> Result<Option<SmartContractValue>, Self::Error> {
        let find_key = (key.chain_id.clone(), key.address.to_string());
        let find_result = contract_url::Entity::find_by_id(find_key)
            .one(&self.db)
            .await?;
        find_result
            .map(|contract| {
                Ok(SmartContractValue {
                    blockscout_url: url::Url::parse(&contract.url)?,
                    sources: BTreeMap::new(),
                })
            })
            .transpose()
    }

    async fn remove(
        &self,
        key: &SmartContractId,
    ) -> Result<Option<SmartContractValue>, Self::Error> {
        todo!()
    }
}
