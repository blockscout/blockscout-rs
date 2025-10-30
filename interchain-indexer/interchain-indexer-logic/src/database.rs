use std::sync::Arc;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use interchain_indexer_entity::{chains, bridges, bridge_contracts};

pub struct InterchainDatabase {
    db: Arc<DatabaseConnection>,
    
}

impl InterchainDatabase {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self { db }
    }

    pub async fn get_all_chains(&self) -> anyhow::Result<Vec<chains::Model>> {
        match chains::Entity::find()
            .all(self.db.as_ref())
            .await {
                Ok(result) => Ok(result),

                Err(e) => {
                    tracing::error!(err =? e, "Failed to fetch all chains");
                    Err(e.into())
                }
            }
    }

    pub async fn get_all_bridges(&self) -> anyhow::Result<Vec<bridges::Model>> {
        match bridges::Entity::find()
            .all(self.db.as_ref())
            .await {
                Ok(result) => Ok(result),
                Err(e) => {
                    tracing::error!(err =? e, "Failed to fetch all bridges");
                    Err(e.into())
                }
            }
    }

    pub async fn get_bridge_contracts(&self, bridge_id: u64) -> anyhow::Result<Vec<bridge_contracts::Model>> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .all(self.db.as_ref())
            .await {
                Ok(result) => Ok(result),
                Err(e) => {
                    tracing::error!(err =? e, "Failed to fetch bridge contracts");
                    Err(e.into())
                }
            }
    }

    pub async fn get_bridge_contract(&self, bridge_id: u64, chain_id: u64) -> anyhow::Result<bridge_contracts::Model> {
        match bridge_contracts::Entity::find()
            .filter(bridge_contracts::Column::BridgeId.eq(bridge_id))
            .filter(bridge_contracts::Column::ChainId.eq(chain_id))
            .one(self.db.as_ref())
            .await {
                Ok(Some(result)) => Ok(result),
                Ok(None) => {
                    let err_msg = format!("No bridge contract found for bridge_id={} and chain_id={}", bridge_id, chain_id);
                    tracing::error!("{}", err_msg);
                    Err(anyhow::anyhow!(err_msg))
                },
                Err(e) => {
                    tracing::error!(err =? e, "Failed to fetch bridge contract");
                    Err(e.into())
                }
            }
    }

}

#[cfg(test)]
mod tests {
    use crate::{InterchainDatabase, test_utils::{init_db, mock_db::fill_mock_interchain_database}};
    use sea_orm::EntityName;
    use std::sync::Arc;

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
    }
}
