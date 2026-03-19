//! Orchestration layer for statistics: projection triggers, backfill, rollup refresh, token enrichment.

use std::sync::Arc;

use interchain_indexer_entity::crosschain_transfers;
use sea_orm::{
    ActiveValue, ColumnTrait, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, sea_query::Expr,
};

use crate::{
    InterchainDatabase, TokenInfoService,
    message_buffer::{ConsolidatedMessage, token_keys_from_finalized_for_enrichment},
};

/// Coordinates stats-related workflows on top of [`InterchainDatabase`].
///
/// Token metadata enrichment is optional: when [`Self::token_info`] is absent, projection and
/// rollups still run; only async enrichment kickoffs are skipped.
///
/// Read-side helpers for future APIs (for example gRPC list endpoints) can be added here and
/// delegate to [`InterchainDatabase`].
pub struct StatsService {
    db: Arc<InterchainDatabase>,
    token_info: Option<Arc<TokenInfoService>>,
}

impl StatsService {
    pub fn new(db: Arc<InterchainDatabase>, token_info: Option<Arc<TokenInfoService>>) -> Self {
        Self { db, token_info }
    }

    pub fn interchain_db(&self) -> &InterchainDatabase {
        self.db.as_ref()
    }

    pub fn interchain_db_arc(&self) -> Arc<InterchainDatabase> {
        self.db.clone()
    }

    pub fn token_info(&self) -> Option<&Arc<TokenInfoService>> {
        self.token_info.as_ref()
    }

    /// Inline stats projection for finalized batches (same DB transaction as flush).
    pub async fn apply_stats_for_finalized_batch(
        &self,
        tx: &DatabaseTransaction,
        finalized: &[ConsolidatedMessage],
    ) -> Result<(), DbErr> {
        if finalized.is_empty() {
            return Ok(());
        }
        let mut msg_pks = Vec::with_capacity(finalized.len());
        for c in finalized {
            let (mid, brid) = match (&c.message.id, &c.message.bridge_id) {
                (ActiveValue::Set(mid), ActiveValue::Set(brid)) => (*mid, *brid),
                _ => {
                    return Err(DbErr::Custom(
                        "finalized consolidated message must have id and bridge_id set".into(),
                    ));
                }
            };
            msg_pks.push((mid, brid));
        }

        super::projection::project_messages_batch(tx, &msg_pks).await?;

        let transfer_ids: Vec<i64> = crosschain_transfers::Entity::find()
            .filter(
                Expr::tuple([
                    Expr::col(crosschain_transfers::Column::MessageId).into(),
                    Expr::col(crosschain_transfers::Column::BridgeId).into(),
                ])
                .in_tuples(msg_pks.iter().copied()),
            )
            .filter(crosschain_transfers::Column::StatsProcessed.eq(0i16))
            .all(tx)
            .await?
            .into_iter()
            .map(|t| t.id)
            .collect();

        super::projection::project_transfers_batch(tx, &transfer_ids).await?;
        Ok(())
    }

    pub async fn recompute_stats_chains(&self) -> anyhow::Result<()> {
        self.db.recompute_stats_chains().await
    }

    pub async fn backfill_stats_until_idle(&self) -> anyhow::Result<()> {
        self.db.backfill_stats_until_idle().await
    }

    pub async fn backfill_stats_until_idle_with_token_enrichment(&self) -> anyhow::Result<()> {
        self.db
            .backfill_stats_until_idle_with_token_enrichment(self.token_info.clone())
            .await
    }

    /// Triggers async token metadata fetch for stats tables (no-op without token service).
    pub fn kickoff_token_enrichment_for_keys(&self, keys: Vec<(i64, Vec<u8>)>) {
        if keys.is_empty() {
            return;
        }
        if let Some(svc) = self.token_info.as_ref() {
            svc.clone().kickoff_token_fetch_for_stats_enrichment(keys);
        }
    }

    pub fn kickoff_token_enrichment_for_finalized(&self, finalized: &[ConsolidatedMessage]) {
        let keys = token_keys_from_finalized_for_enrichment(finalized);
        self.kickoff_token_enrichment_for_keys(keys);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::init_db;

    #[tokio::test]
    #[ignore = "needs database"]
    async fn kickoff_enrichment_no_token_service_is_noop() {
        let guard = init_db("stats_service_kickoff_no_token").await;
        let db = Arc::new(InterchainDatabase::new(guard.client()));
        let stats = StatsService::new(db, None);
        stats.kickoff_token_enrichment_for_keys(vec![(1, vec![0xab; 20])]);
    }
}
