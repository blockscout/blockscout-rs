use std::collections::BTreeMap;

use basic_cache_entity::{contract_sources, contract_url};

use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    Iterable, QueryFilter,
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

impl PostgresCache {
    async fn set_url(&self, chain_id: String, address: String, url: String) -> Result<(), Error> {
        contract_url::Entity::insert(contract_url::ActiveModel {
            chain_id: Set(chain_id),
            address: Set(address),
            url: Set(url),
        })
        .on_conflict(
            OnConflict::columns(contract_url::PrimaryKey::iter())
                .update_column(contract_url::Column::Url)
                .to_owned(),
        )
        .exec(&self.db)
        .await?;
        Ok(())
    }

    async fn get_url(&self, chain_id: String, address: String) -> Result<Option<url::Url>, Error> {
        let find_key = (chain_id, address);
        let find_result = contract_url::Entity::find_by_id(find_key)
            .one(&self.db)
            .await?;
        find_result
            .map(|contract| Ok(url::Url::parse(&contract.url)?))
            .transpose()
    }

    async fn remove_url(&self, chain_id: String, address: String) -> Result<(), Error> {
        let find_key = (chain_id, address);
        let find_result = contract_url::Entity::delete_by_id(find_key)
            .exec(&self.db)
            .await?;
        if find_result.rows_affected > 1 {
            tracing::warn!(
                "unexpected number of removed urls: {}",
                find_result.rows_affected
            );
        }
        Ok(())
    }

    async fn set_sources(
        &self,
        chain_id: String,
        address: String,
        sources: impl IntoIterator<Item = (String, String)>,
    ) -> Result<(), Error> {
        // since we overwrite existing contracts, we need to prune old sources
        self.remove_sources(chain_id.clone(), address.clone())
            .await?;

        let sources_models =
            sources
                .into_iter()
                .map(|(filename, contents)| contract_sources::ActiveModel {
                    chain_id: Set(chain_id.clone()),
                    address: Set(address.clone()),
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

    async fn get_sources(
        &self,
        chain_id: String,
        address: String,
    ) -> Result<BTreeMap<String, String>, Error> {
        let models = contract_sources::Entity::find()
            .filter(contract_sources::Column::ChainId.eq(chain_id))
            .filter(contract_sources::Column::Address.eq(address))
            .all(&self.db)
            .await?;
        Ok(models
            .into_iter()
            .map(|m| (m.filename, m.contents))
            .collect())
    }

    async fn remove_sources(&self, chain_id: String, address: String) -> Result<(), Error> {
        contract_sources::Entity::delete_many()
            .filter(contract_sources::Column::ChainId.eq(chain_id))
            .filter(contract_sources::Column::Address.eq(address))
            .exec(&self.db)
            .await?;
        Ok(())
    }
}

impl CacheManager<SmartContractId, SmartContractValue> for PostgresCache {
    type Error = Error;

    async fn set(
        &self,
        key: SmartContractId,
        value: SmartContractValue,
    ) -> Result<(), Self::Error> {
        self.set_url(
            key.chain_id.clone(),
            key.address.to_string(),
            value.blockscout_url.to_string(),
        )
        .await?;
        self.set_sources(key.chain_id.clone(), key.address.to_string(), value.sources)
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
        let url = self
            .get_url(key.chain_id.clone(), key.address.to_string())
            .await?;
        let sources = self
            .get_sources(key.chain_id.clone(), key.address.to_string())
            .await?;

        let Some(url) = url else {
            if !sources.is_empty() {
                tracing::warn!(
                    "detected {} dangling source files for contract {:?}",
                    sources.len(),
                    (&key.chain_id, key.address.to_string())
                );
            }
            return Ok(None);
        };

        Ok(Some(SmartContractValue {
            blockscout_url: url,
            sources,
        }))
    }

    async fn remove(
        &self,
        key: &SmartContractId,
    ) -> Result<Option<SmartContractValue>, Self::Error> {
        let contract = self.get(key).await?;
        self.remove_url(key.chain_id.clone(), key.address.to_string())
            .await?;
        self.remove_sources(key.chain_id.clone(), key.address.to_string())
            .await?;
        Ok(contract)
    }
}
