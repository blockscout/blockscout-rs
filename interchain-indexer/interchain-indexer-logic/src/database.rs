use interchain_indexer_entity::{
    bridge_contracts, bridges, chains, crosschain_messages, crosschain_transfers,
    indexer_checkpoints,
};
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    TransactionTrait, prelude::Expr, sea_query::OnConflict,
};
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct InterchainDatabase {
    pub db: Arc<DatabaseConnection>,
}

impl InterchainDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    // CONFIGURATION TABLE: chains
    pub async fn upsert_chains(&self, chains: Vec<chains::ActiveModel>) -> anyhow::Result<()> {
        if chains.is_empty() {
            return Ok(());
        }

        match chains::Entity::insert_many(chains)
            .on_conflict(
                OnConflict::column(chains::Column::Id)
                    .update_columns([
                        chains::Column::Name,
                        chains::Column::NativeId,
                        chains::Column::Icon,
                    ])
                    .value(chains::Column::UpdatedAt, Expr::current_timestamp())
                    .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(err =? e, "Failed to upsert chains");
                Err(e.into())
            }
        }
    }

    pub async fn get_all_chains(&self) -> anyhow::Result<Vec<chains::Model>> {
        match chains::Entity::find().all(self.db.as_ref()).await {
            Ok(result) => Ok(result),

            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch all chains");
                Err(e.into())
            }
        }
    }

    /// Load a map of native blockchain IDs (normalized with 0x prefix) to chain id.
    ///
    /// This is useful for pre-populating per-batch caches so handlers don't need to
    /// hit the database for every log. Only chains with a non-null `native_id` are
    /// included.
    pub async fn load_native_id_map(&self) -> anyhow::Result<HashMap<String, i64>> {
        chains::Entity::find()
            .filter(chains::Column::NativeId.is_not_null())
            .all(self.db.as_ref())
            .await
            .map(|rows| {
                rows.into_iter()
                    .filter_map(|row| (row.native_id?, row.id).into())
                    .collect::<HashMap<_, _>>()
            })
            .map_err(|e| {
                tracing::error!(err =? e, "Failed to load native id -> chain id map");
                e.into()
            })
    }

    // CONFIGURATION TABLE: bridges
    // Updating the name of a bridge with an existing ID is prohibited
    // Renaming a bridge is allowed only via a direct SQL request
    pub async fn upsert_bridges(&self, bridges: Vec<bridges::ActiveModel>) -> anyhow::Result<()> {
        // Extract id and name from input bridges for validation
        let bridge_id_name_map: HashMap<i32, String> = bridges
            .iter()
            .filter_map(|bridge| match (&bridge.id, &bridge.name) {
                (ActiveValue::Set(id), ActiveValue::Set(name)) => Some((*id, name.clone())),
                _ => None,
            })
            .collect();

        // Check existing bridges and validate id+name match
        let bridge_ids: Vec<i32> = bridge_id_name_map.keys().copied().collect();
        if !bridge_ids.is_empty() {
            match bridges::Entity::find()
                .filter(bridges::Column::Id.is_in(bridge_ids))
                .all(self.db.as_ref())
                .await
            {
                Ok(existing_bridges) => {
                    for existing in existing_bridges {
                        if let Some(expected_name) = bridge_id_name_map.get(&existing.id)
                            && existing.name != *expected_name
                        {
                            let err_msg = format!(
                                "Bridge with id {} exists but has different name: expected '{}', found '{}'",
                                existing.id, expected_name, existing.name
                            );
                            tracing::error!("{}", err_msg);
                            return Err(anyhow::anyhow!(err_msg));
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(err =? e, "Failed to check existing bridges");
                    return Err(e.into());
                }
            }
        }

        self.db
            .transaction::<_, (), DbErr>(|tx| {
                Box::pin(async move {
                    // First, disable all existing bridges
                    // The upsert below will set the appropriate enabled flags for bridges in the input list
                    bridges::Entity::update_many()
                        .col_expr(bridges::Column::Enabled, Expr::value(false))
                        .exec(tx)
                        .await?;

                    // Next proceed with upsert (if any)
                    if !bridges.is_empty() {
                        bridges::Entity::insert_many(bridges)
                            .on_conflict(
                                OnConflict::column(bridges::Column::Id)
                                    .update_columns([
                                        bridges::Column::Type,
                                        bridges::Column::Enabled,
                                        bridges::Column::ApiUrl,
                                        bridges::Column::UiUrl,
                                    ])
                                    .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    Ok(())
                })
            })
            .await?;

        Ok(())
    }

    pub async fn get_all_bridges(&self) -> anyhow::Result<Vec<bridges::Model>> {
        match bridges::Entity::find().all(self.db.as_ref()).await {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch all bridges");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge(&self, bridge_id: i32) -> anyhow::Result<Option<bridges::Model>> {
        match bridges::Entity::find()
            .filter(bridges::Column::Id.eq(bridge_id))
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(result)) => Ok(Some(result)),
            Ok(None) => {
                tracing::error!(bridge_id =? bridge_id, "Bridge not found");
                Ok(None)
            }
            Err(e) => {
                tracing::error!(err =? e, bridge_id =? bridge_id, "Failed to fetch the bridge");
                Err(e.into())
            }
        }
    }

    // CONFIGURATION TABLE: bridge_contracts
    pub async fn upsert_bridge_contracts(
        &self,
        bridge_contracts: Vec<bridge_contracts::ActiveModel>,
    ) -> anyhow::Result<()> {
        if bridge_contracts.is_empty() {
            return Ok(());
        }

        match bridge_contracts::Entity::insert_many(bridge_contracts)
            .on_conflict(
                OnConflict::columns([
                    bridge_contracts::Column::BridgeId,
                    bridge_contracts::Column::ChainId,
                    bridge_contracts::Column::Address,
                    bridge_contracts::Column::Version,
                ])
                .update_columns([
                    bridge_contracts::Column::Abi,
                    bridge_contracts::Column::StartedAtBlock,
                ])
                .value(
                    bridge_contracts::Column::UpdatedAt,
                    Expr::current_timestamp(),
                )
                .to_owned(),
            )
            .exec(self.db.as_ref())
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::error!(err =? e, "Failed to upsert bridge contracts");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge_contracts(
        &self,
        bridge_id: i32,
    ) -> anyhow::Result<Vec<bridge_contracts::Model>> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch bridge contracts");
                Err(e.into())
            }
        }
    }

    pub async fn get_bridge_contract(
        &self,
        bridge_id: i32,
        chain_id: i64,
    ) -> anyhow::Result<bridge_contracts::Model> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .filter(bridge_contracts::Column::ChainId.eq(chain_id))
            .one(self.db.as_ref())
            .await
        {
            Ok(Some(result)) => Ok(result),
            Ok(None) => {
                let err_msg = format!(
                    "No bridge contract found for bridge_id={} and chain_id={}",
                    bridge_id, chain_id
                );
                tracing::error!("{}", err_msg);
                Err(anyhow::anyhow!(err_msg))
            }
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch bridge contract");
                Err(e.into())
            }
        }
    }

    // VIEW TABLE: crosschain_messages
    // TBD: add pagination, filters, etc. Current implementation is just for tests only
    pub async fn get_crosschain_messages(&self) -> anyhow::Result<Vec<crosschain_messages::Model>> {
        match crosschain_messages::Entity::find()
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch crosschain messages");
                Err(e.into())
            }
        }
    }

    // VIEW TABLE: crosschain_transfers
    // TBD: add pagination, filters, etc. Current implementation is just for tests only
    pub async fn get_crosschain_transfers(
        &self,
    ) -> anyhow::Result<Vec<crosschain_transfers::Model>> {
        match crosschain_transfers::Entity::find()
            .all(self.db.as_ref())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                tracing::error!(err =? e, "Failed to fetch crosschain transfers");
                Err(e.into())
            }
        }
    }

    // INDEXER TABLE: indexer_checkpoints
    /// Get checkpoint for a specific bridge and chain
    pub async fn get_checkpoint(
        &self,
        bridge_id: u64,
        chain_id: u64,
    ) -> anyhow::Result<Option<indexer_checkpoints::Model>> {
        indexer_checkpoints::Entity::find()
            .filter(indexer_checkpoints::Column::BridgeId.eq(bridge_id))
            .filter(indexer_checkpoints::Column::ChainId.eq(chain_id))
            .one(self.db.as_ref())
            .await
            .inspect_err(|e| tracing::error!(err =? e, "failed to query checkpoint from database"))
            .map_err(|e| e.into())
    }
}

#[cfg(test)]
mod tests {
    use interchain_indexer_entity::chains;
    use sea_orm::ActiveValue::Set;

    use crate::{
        InterchainDatabase,
        test_utils::{init_db, mock_db::fill_mock_interchain_database},
    };

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn mock_db_works() {
        let db = init_db("mock_db_works").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 2);

        let bridges = interchain_db.get_all_bridges().await.unwrap();
        assert_eq!(bridges.len(), 1);

        let bridge_contracts = interchain_db
            .get_bridge_contracts(bridges[0].id)
            .await
            .unwrap();
        assert_eq!(bridge_contracts.len(), 2);

        let bridge_contract = interchain_db
            .get_bridge_contract(bridges[0].id, chains[0].id)
            .await
            .unwrap();
        assert_eq!(bridge_contract.id, bridge_contracts[0].id);
        assert_eq!(bridge_contract.chain_id, chains[0].id);
        assert_eq!(bridge_contract.bridge_id, bridges[0].id);
        assert_eq!(bridge_contract.address, bridge_contracts[0].address);

        let crosschain_messages = interchain_db.get_crosschain_messages().await.unwrap();
        assert_eq!(crosschain_messages.len(), 4);

        let crosschain_transfers = interchain_db.get_crosschain_transfers().await.unwrap();
        assert_eq!(crosschain_transfers.len(), 5);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn mock_db_upsert_chain() {
        let db = init_db("mock_db_upsert_chain").await;
        fill_mock_interchain_database(&db).await;

        let interchain_db = InterchainDatabase::new(db.client());

        let mut ava_chain = chains::ActiveModel {
            id: Set(43114),
            name: Set("C-Chain".to_string()),
            native_id: Set(Some(
                "2q9e4r6Mu3U68nU1fYjgbR6JvwrRx36CohpAX5UQxse55x1Q5".to_string(),
            )),
            icon: Set(Some(
                "https://chainlist.org/chain/43114/icon.png".to_string(),
            )),
            ..Default::default()
        };

        interchain_db.upsert_chains(vec![]).await.unwrap();
        interchain_db
            .upsert_chains(vec![ava_chain.clone()])
            .await
            .unwrap();

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 3);

        ava_chain.name = Set("Avalanche C-Chain".to_string());
        interchain_db
            .upsert_chains(vec![ava_chain.clone()])
            .await
            .unwrap();

        let chains = interchain_db.get_all_chains().await.unwrap();
        assert_eq!(chains.len(), 3);
        let stored_chain = chains.iter().find(|chain| chain.id == 43114).unwrap();
        assert_eq!(stored_chain.name, ava_chain.name.unwrap());
        assert_eq!(stored_chain.native_id, ava_chain.native_id.unwrap());
        assert_eq!(stored_chain.icon, ava_chain.icon.unwrap());
    }
}
