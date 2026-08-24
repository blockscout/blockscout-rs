// SPDX-License-Identifier: LicenseRef-Blockscout

//! Orchestration layer for statistics: projection triggers, backfill, rollup refresh, token enrichment.

use std::sync::Arc;

use interchain_indexer_entity::crosschain_transfers;
use sea_orm::{ActiveValue, DatabaseTransaction, DbErr, EntityTrait, QueryFilter, sea_query::Expr};

use super::{IndexedChains, StatsListQuery};
use crate::{
    BridgedTokenAggDbRow, BridgedTokenLinkEnriched, InterchainDatabase, TokenInfoService,
    message_buffer::{ConsolidatedMessage, token_keys_from_flushed_for_enrichment},
    pagination::{
        BridgedTokensPaginationLogic, BridgedTokensSortField, OutputPagination,
        StatsChainsPaginationLogic, StatsChainsSortField,
    },
    stats_chains_query::StatsChainListRow,
};

/// One row of `/stats/bridged-tokens` after joining `stats_asset_tokens` + `tokens`.
#[derive(Debug, Clone)]
pub struct BridgedTokenListRow {
    pub aggregate: BridgedTokenAggDbRow,
    pub tokens: Vec<BridgedTokenLinkEnriched>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatsReadSettings {
    pub include_zero_chains: bool,
}

impl Default for StatsReadSettings {
    fn default() -> Self {
        Self {
            include_zero_chains: true,
        }
    }
}

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
    read_settings: StatsReadSettings,
    indexed_chains: IndexedChains,
}

impl StatsService {
    pub fn new(
        db: Arc<InterchainDatabase>,
        token_info: Option<Arc<TokenInfoService>>,
        read_settings: StatsReadSettings,
        indexed_chains: IndexedChains,
    ) -> Self {
        Self {
            db,
            token_info,
            read_settings,
            indexed_chains,
        }
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

    pub fn read_settings(&self) -> StatsReadSettings {
        self.read_settings
    }

    /// The stats layer's single observability-horizon input: which chains are
    /// indexed per bridge. See [`IndexedChains::may_observe`].
    pub fn indexed_chains(&self) -> &IndexedChains {
        &self.indexed_chains
    }

    /// Inline stats projection for a flushed batch (same DB transaction as the
    /// flush itself). Takes **all** flushed entries — final and `Partial` —
    /// not only finalized ones: identity maintenance (linking a newly known
    /// token endpoint, merging two asset components) must run for every
    /// flushed canonical key so a later relink is not silently missed, while
    /// counting stays gated on [`super::projection::project_transfers_batch`]'s
    /// own `stats_processed` / eligibility rule. See
    /// `.memory-bank/gotchas.md` "Stats Eligibility Is About Observability,
    /// Not Protocol Terminality".
    pub async fn apply_stats_for_flushed_batch(
        &self,
        tx: &DatabaseTransaction,
        flushed: &[ConsolidatedMessage],
    ) -> Result<(), DbErr> {
        if flushed.is_empty() {
            return Ok(());
        }
        let mut msg_pks = Vec::with_capacity(flushed.len());
        for c in flushed {
            let (mid, brid) = match (&c.message.id, &c.message.bridge_id) {
                (ActiveValue::Set(mid), ActiveValue::Set(brid)) => (*mid, *brid),
                _ => {
                    return Err(DbErr::Custom(
                        "flushed consolidated message must have id and bridge_id set".into(),
                    ));
                }
            };
            msg_pks.push((mid, brid));
        }

        super::projection::project_messages_batch(tx, &msg_pks, &self.indexed_chains).await?;

        // No `stats_processed = 0` filter here: an already-counted transfer of
        // a flushed key must still reach `project_transfers_batch`, which
        // itself decides identity-maintenance-only (repair) vs. counting.
        //
        // Chunked at two bind params per key: `flushed` is the whole
        // consolidatable cohort of one maintenance cycle, which during catch-up
        // can exceed the PostgreSQL bind-param limit on its own. Read loops that
        // accumulate results chunk by hand rather than through
        // `bulk::run_in_batches`, whose closure cannot lend out a mutable
        // accumulator — see `projection.rs`'s `load_*_map` helpers.
        let batch_size = (crate::bulk::PG_BIND_PARAM_LIMIT / 2).max(1);
        let mut transfer_ids: Vec<i64> = Vec::with_capacity(msg_pks.len());
        for batch in msg_pks.chunks(batch_size) {
            let found = crosschain_transfers::Entity::find()
                .filter(
                    Expr::tuple([
                        Expr::col(crosschain_transfers::Column::MessageId).into(),
                        Expr::col(crosschain_transfers::Column::BridgeId).into(),
                    ])
                    .in_tuples(batch.iter().copied()),
                )
                .all(tx)
                .await?;
            transfer_ids.extend(found.into_iter().map(|t| t.id));
        }

        super::projection::project_transfers_batch(tx, &transfer_ids, &self.indexed_chains).await?;
        Ok(())
    }

    /// Refreshes both `stats_chains` and `stats_chains_by_bridge` in one
    /// transaction and publishes the bridge-sum overcount gauges from the
    /// resulting report only after that commit succeeds — see
    /// `stats/metrics.rs` and `.memory-bank/rules/testing.md`'s ban on
    /// asserting process-wide metric deltas (the report/database state is
    /// what tests should assert instead).
    pub async fn recompute_stats_chains(
        &self,
    ) -> anyhow::Result<crate::database::StatsChainsRecomputeReport> {
        let report = self.db.recompute_stats_chains().await?;
        super::metrics::STATS_CHAINS_BRIDGE_SUM_OVERCOUNT_USERS
            .with_label_values(&["transfer"])
            .set(report.transfer_overcount_users as f64);
        super::metrics::STATS_CHAINS_BRIDGE_SUM_OVERCOUNT_USERS
            .with_label_values(&["message"])
            .set(report.message_overcount_users as f64);
        super::metrics::STATS_CHAINS_BRIDGE_SUM_AFFECTED_CHAINS
            .with_label_values(&["transfer"])
            .set(report.transfer_affected_chains as f64);
        super::metrics::STATS_CHAINS_BRIDGE_SUM_AFFECTED_CHAINS
            .with_label_values(&["message"])
            .set(report.message_affected_chains as f64);
        Ok(report)
    }

    pub async fn backfill_stats_until_idle(&self) -> anyhow::Result<()> {
        self.db
            .backfill_stats_until_idle(&self.indexed_chains)
            .await
    }

    pub async fn backfill_stats_until_idle_with_token_enrichment(&self) -> anyhow::Result<()> {
        self.db
            .backfill_stats_until_idle_with_token_enrichment(
                &self.indexed_chains,
                self.token_info.clone(),
            )
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

    /// Kicks off token metadata fetch for every consolidated entry in the
    /// batch, not only finalized ones — a transfer to an unindexed chain is
    /// never `is_final` but is now countable and asset-linked, so it must
    /// still be eligible for enrichment (task.md Success Criteria: "assets
    /// created from unindexed-counterpart transfers are eligible for token
    /// metadata enrichment").
    pub fn kickoff_token_enrichment_for_flushed(&self, flushed: &[ConsolidatedMessage]) {
        let keys = token_keys_from_flushed_for_enrichment(flushed);
        self.kickoff_token_enrichment_for_keys(keys);
    }

    /// Bridged-token stats table for a chain: aggregated edges + full token list per asset.
    ///
    /// `indexed_pairs` / `indexed_union` are taken as parameters rather than read
    /// off `self.indexed_chains()`: the read-side default-hide filter is a
    /// per-request opt-in (`include_unindexed_chains`), not a property of the
    /// service. Callers pass `None` for both to apply no restriction (opt-in
    /// requested, or an `AllIndexed` configuration).
    #[allow(clippy::too_many_arguments)]
    pub async fn get_bridged_tokens_for_chain(
        &self,
        chain_id: i64,
        counterparty_chain_ids: Option<&[i64]>,
        bridge_ids: Option<&[i32]>,
        indexed_pairs: Option<&[(i32, Vec<i64>)]>,
        indexed_union: Option<&[i64]>,
        params: StatsListQuery<'_, BridgedTokensSortField, BridgedTokensPaginationLogic>,
    ) -> anyhow::Result<(
        Vec<BridgedTokenListRow>,
        OutputPagination<BridgedTokensPaginationLogic>,
    )> {
        let (rows, pagination) = self
            .db
            .list_bridged_token_stats_for_chain(
                chain_id,
                counterparty_chain_ids,
                bridge_ids,
                indexed_pairs,
                params,
            )
            .await?;

        let ids: Vec<i64> = rows.iter().map(|r| r.stats_asset_id).collect();
        let by_asset = self
            .db
            .fetch_bridged_token_items_for_assets(&ids, indexed_union)
            .await?;

        let out = rows
            .into_iter()
            .map(|agg| {
                let tokens = by_asset
                    .get(&agg.stats_asset_id)
                    .cloned()
                    .unwrap_or_default();
                BridgedTokenListRow {
                    aggregate: agg,
                    tokens,
                }
            })
            .collect();

        Ok((out, pagination))
    }

    /// Known chains with `unique_transfer_users_count`, from the exact global
    /// `stats_chains` snapshot (`scope = Global`) or summed from the selected
    /// bridges' `stats_chains_by_bridge` cells (`scope = Bridges`); 0 when
    /// missing.
    ///
    /// `indexed_chain_ids` is a per-request parameter, not read off
    /// `self.indexed_chains()` — see [`Self::get_bridged_tokens_for_chain`].
    pub async fn get_stats_chains(
        &self,
        scope: crate::stats_chains_query::StatsChainsScope<'_>,
        chain_ids: Vec<i64>,
        indexed_chain_ids: Option<&[i64]>,
        params: StatsListQuery<'_, StatsChainsSortField, StatsChainsPaginationLogic>,
    ) -> anyhow::Result<(
        Vec<StatsChainListRow>,
        OutputPagination<StatsChainsPaginationLogic>,
    )> {
        self.db
            .list_stats_chains(
                scope,
                chain_ids.as_slice(),
                self.read_settings.include_zero_chains,
                indexed_chain_ids,
                params,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_outgoing_message_paths(
        &self,
        chain_id: i64,
        from_date: Option<chrono::NaiveDate>,
        to_date: Option<chrono::NaiveDate>,
        counterparty_chain_ids: Option<&[i64]>,
        bridge_ids: Option<&[i32]>,
        indexed_pairs: Option<&[(i32, Vec<i64>)]>,
        indexed_chain_ids: Option<&[i64]>,
    ) -> anyhow::Result<Vec<crate::MessagePathStatsRow>> {
        self.db
            .get_outgoing_message_paths(
                chain_id,
                from_date,
                to_date,
                counterparty_chain_ids,
                bridge_ids,
                self.read_settings.include_zero_chains,
                indexed_pairs,
                indexed_chain_ids,
            )
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn get_incoming_message_paths(
        &self,
        chain_id: i64,
        from_date: Option<chrono::NaiveDate>,
        to_date: Option<chrono::NaiveDate>,
        counterparty_chain_ids: Option<&[i64]>,
        bridge_ids: Option<&[i32]>,
        indexed_pairs: Option<&[(i32, Vec<i64>)]>,
        indexed_chain_ids: Option<&[i64]>,
    ) -> anyhow::Result<Vec<crate::MessagePathStatsRow>> {
        self.db
            .get_incoming_message_paths(
                chain_id,
                from_date,
                to_date,
                counterparty_chain_ids,
                bridge_ids,
                self.read_settings.include_zero_chains,
                indexed_pairs,
                indexed_chain_ids,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use interchain_indexer_entity::{
        bridges, chains, crosschain_messages, sea_orm_active_enums::MessageStatus,
    };
    use sea_orm::TransactionTrait;

    use super::*;
    use crate::test_utils::init_db;

    #[tokio::test]
    #[ignore = "needs database"]
    async fn kickoff_enrichment_no_token_service_is_noop() {
        let guard = init_db("stats_service_kickoff_no_token").await;
        let db = Arc::new(InterchainDatabase::new(guard.client()));
        let stats = StatsService::new(
            db,
            None,
            StatsReadSettings::default(),
            IndexedChains::AllIndexed,
        );
        stats.kickoff_token_enrichment_for_keys(vec![(1, vec![0xab; 20])]);
    }

    /// coding-task-4b work item 1: a non-final (`Partial`) consolidation must
    /// still reach the stats hook. A message to a chain unindexed for its
    /// bridge is countable per `message_countable_condition` regardless of
    /// `is_final` (that flag is an ICTT-completion detail the stats layer
    /// does not consult), so this is the scenario the old `is_final`-only
    /// filter used to silently drop forever.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_partial_flush_reaches_stats_hook_and_counts_unindexed_destination() {
        let guard = init_db("stats_service_partial_flush_reaches_hook").await;
        let conn = guard.client();

        let raw_db = InterchainDatabase::new(conn.clone());
        raw_db
            .upsert_bridges(vec![bridges::ActiveModel {
                id: ActiveValue::Set(1),
                name: ActiveValue::Set("test_bridge".into()),
                enabled: ActiveValue::Set(true),
                ..Default::default()
            }])
            .await
            .unwrap();
        raw_db
            .upsert_chains(vec![
                chains::ActiveModel {
                    id: ActiveValue::Set(1),
                    name: ActiveValue::Set("src".into()),
                    ..Default::default()
                },
                chains::ActiveModel {
                    id: ActiveValue::Set(900),
                    name: ActiveValue::Set("unindexed_dst".into()),
                    ..Default::default()
                },
            ])
            .await
            .unwrap();

        // Seed the canonical row as if a Partial flush had already written it
        // (mirroring what `flush_to_final_storage` does before the stats hook
        // runs in the same transaction).
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: ActiveValue::Set(1),
            bridge_id: ActiveValue::Set(1),
            status: ActiveValue::Set(MessageStatus::Initiated),
            init_timestamp: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            src_chain_id: ActiveValue::Set(1),
            dst_chain_id: ActiveValue::Set(Some(900)),
            src_tx_hash: ActiveValue::Set(Some(vec![0xab; 32])),
            stats_processed: ActiveValue::Set(0),
            ..Default::default()
        })
        .exec(conn.as_ref())
        .await
        .unwrap();

        // Chain 900 is unindexed for bridge 1 (present in the map, absent
        // from its set) -> the message is countable despite being `Initiated`.
        let stats = Arc::new(StatsService::new(
            Arc::new(InterchainDatabase::new(conn.clone())),
            None,
            StatsReadSettings::default(),
            IndexedChains::from_pairs([(1, 1)]),
        ));

        let partial = ConsolidatedMessage {
            is_final: false,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(1),
                bridge_id: ActiveValue::Set(1),
                ..Default::default()
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        };

        conn.transaction::<_, (), DbErr>(|tx| {
            let stats = stats.clone();
            Box::pin(async move { stats.apply_stats_for_flushed_batch(tx, &[partial]).await })
        })
        .await
        .unwrap();

        let row = crosschain_messages::Entity::find_by_id((1i64, 1i32))
            .one(conn.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            row.stats_processed, 1,
            "a Partial entry to an unindexed destination must still reach the stats hook and count"
        );
    }
}
