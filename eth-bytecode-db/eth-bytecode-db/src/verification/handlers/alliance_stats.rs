use super::super::Client;
use anyhow::Error;
use sea_orm::{ColumnTrait, EntityTrait, FromQueryResult, QuerySelect};
use std::collections::BTreeMap;
use verifier_alliance_entity::verified_contracts;

pub struct Stats {
    pub total_contracts: u64,
    pub contracts_per_provider: BTreeMap<String, u64>,
}

#[derive(FromQueryResult)]
struct ProviderContractsCount {
    created_by: String,
    count: u64,
}

pub async fn stats(client: Client) -> Result<Option<Stats>, Error> {
    if let Some(alliance_db_client) = client.alliance_db_client {
        let contracts_per_provider: BTreeMap<String, u64> = verified_contracts::Entity::find()
            .select_only()
            .column(verified_contracts::Column::CreatedBy)
            .column_as(verified_contracts::Column::Id.count(), "count")
            .group_by(verified_contracts::Column::CreatedBy)
            .into_model::<ProviderContractsCount>()
            .all(alliance_db_client.as_ref())
            .await?
            .into_iter()
            .map(|value| (value.created_by, value.count))
            .collect();
        let total_contracts = contracts_per_provider.iter().map(|v| v.1).sum();

        Ok(Some(Stats {
            contracts_per_provider,
            total_contracts,
        }))
    } else {
        Ok(None)
    }
}
