// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{collections::HashMap, iter::Sum, ops::Add, time::Instant};

use anyhow::{Context, Result};
use chrono::{TimeDelta, Utc};
use sea_orm::{DbErr, TransactionTrait};

use super::{
    BufferItem, BufferItemVersion, Consolidate, ConsolidatedMessage, Key, MessageBuffer,
    persistence,
};
use crate::message_buffer::{
    cursor::{BridgeId, CursorBlocksBuilder, Cursors},
    metrics,
};

/// Classification of a buffer entry during maintenance planning.
enum ConsolidationOutcome {
    Unchanged,
    NotReady,
    Partial(ConsolidatedMessage),
    Complete(ConsolidatedMessage),
}

#[derive(Clone, Copy, Debug)]
enum HotEvictionReason {
    Stale,
    Finalized,
}

/// Per-bridge statistics for one maintenance cycle.
#[derive(Default, Clone, Copy, Debug)]
struct Counts {
    finalized_messages: usize,
    finalized_transfers: usize,
    hot_entries: usize,
    not_consolidatable: usize,
    stale: usize,
    consolidated_not_final: usize,
    removed_stale: usize,
    removed_finalized: usize,
    skipped_modified: usize,
}

impl Add for Counts {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            finalized_messages: self.finalized_messages + rhs.finalized_messages,
            finalized_transfers: self.finalized_transfers + rhs.finalized_transfers,
            hot_entries: self.hot_entries + rhs.hot_entries,
            not_consolidatable: self.not_consolidatable + rhs.not_consolidatable,
            stale: self.stale + rhs.stale,
            consolidated_not_final: self.consolidated_not_final + rhs.consolidated_not_final,
            removed_stale: self.removed_stale + rhs.removed_stale,
            removed_finalized: self.removed_finalized + rhs.removed_finalized,
            skipped_modified: self.skipped_modified + rhs.skipped_modified,
        }
    }
}

impl Sum for Counts {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), Add::add)
    }
}

/// Aggregated per-bridge statistics for a maintenance cycle.
#[derive(Clone, Debug, Default)]
struct BridgeCounts(HashMap<BridgeId, Counts>);

fn record_bridge_metrics(bridge_id: &BridgeId, stats: &Counts) {
    let bridge_label = bridge_id.to_string();

    for (state, value) in [
        ("not_consolidatable", stats.not_consolidatable),
        ("consolidated_not_final", stats.consolidated_not_final),
        ("stale", stats.stale),
    ] {
        metrics::BUFFER_MAINTENANCE_ENTRIES
            .with_label_values(&[&bridge_label, state])
            .set(value as f64);
    }

    for (reason, value) in [
        ("stale", stats.removed_stale),
        ("finalized", stats.removed_finalized),
    ] {
        metrics::BUFFER_EVICTED_ENTRIES
            .with_label_values(&[&bridge_label, reason])
            .observe(value as f64);
    }
    metrics::BUFFER_EVICTION_SKIPPED_TOTAL
        .with_label_values(&[&bridge_label])
        .inc_by(stats.skipped_modified as u64);

    metrics::BUFFER_MESSAGES_FINALIZED_TOTAL
        .with_label_values(&[&bridge_label])
        .inc_by(stats.finalized_messages as u64);
    metrics::BUFFER_TRANSFERS_FINALIZED_TOTAL
        .with_label_values(&[&bridge_label])
        .inc_by(stats.finalized_transfers as u64);

    metrics::BUFFER_HOT_ENTRIES
        .with_label_values(&[&bridge_label])
        .set(stats.hot_entries as f64);
}

impl BridgeCounts {
    fn entry(&mut self, bridge_id: BridgeId) -> &mut Counts {
        self.0.entry(bridge_id).or_default()
    }

    fn totals(&self) -> Counts {
        self.0.values().copied().sum()
    }

    /// Record all per-bridge maintenance metrics.
    fn record_metrics(&self) {
        for (bridge_id, stats) in &self.0 {
            record_bridge_metrics(bridge_id, stats);
        }
    }
}

fn classify_item<T: Consolidate>(key: &Key, item: &BufferItem<T>) -> Result<ConsolidationOutcome> {
    if !item.is_dirty() {
        return Ok(ConsolidationOutcome::Unchanged);
    }

    match item.inner.consolidate(key)? {
        Some(message) if message.is_final => Ok(ConsolidationOutcome::Complete(message)),
        Some(message) => Ok(ConsolidationOutcome::Partial(message)),
        None => Ok(ConsolidationOutcome::NotReady),
    }
}

#[derive(Clone, Debug, Default)]
struct MaintenancePlan<T: Consolidate + Default> {
    consolidated_entries: Vec<ConsolidatedMessage>,
    stale_entries: Vec<(Key, BufferItem<T>)>,
    finalized_keys: Vec<Key>,
    keys_to_mark_flushed: Vec<(Key, BufferItemVersion)>,
    hot_evictions: Vec<(Key, BufferItemVersion, HotEvictionReason)>,
    cursor_builder: CursorBlocksBuilder,
    stats: BridgeCounts,
}

impl<T: Consolidate + Default> MaintenancePlan<T> {
    fn new() -> Self {
        Self::default()
    }

    fn collect_stale(&mut self, key: Key, item: &BufferItem<T>) {
        self.stale_entries.push((key, item.clone()));
        self.hot_evictions
            .push((key, item.version, HotEvictionReason::Stale));
        self.stats.entry(key.bridge_id).stale += 1;
        self.cursor_builder
            .merge_cold(key.bridge_id, &item.touched_blocks);
    }

    fn collect_hot(&mut self, key: Key, item: &BufferItem<T>) {
        self.stats.entry(key.bridge_id).hot_entries += 1;
        self.cursor_builder
            .merge_hot(key.bridge_id, &item.touched_blocks);
    }

    fn collect(
        &mut self,
        key: Key,
        item: &BufferItem<T>,
        outcome: ConsolidationOutcome,
        is_stale: bool,
    ) {
        let bridge_id = key.bridge_id;

        match outcome {
            ConsolidationOutcome::Unchanged => {}
            ConsolidationOutcome::NotReady => {
                self.stats.entry(bridge_id).not_consolidatable += 1;
            }
            ConsolidationOutcome::Partial(message) => {
                self.consolidated_entries.push(message);
                self.keys_to_mark_flushed.push((key, item.version));
                self.stats.entry(bridge_id).consolidated_not_final += 1;
            }
            ConsolidationOutcome::Complete(message) => {
                let transfer_count = message.transfers.len();
                self.consolidated_entries.push(message);
                self.finalized_keys.push(key);
                self.hot_evictions
                    .push((key, item.version, HotEvictionReason::Finalized));
                let stats = self.stats.entry(bridge_id);
                stats.finalized_messages += 1;
                stats.finalized_transfers += transfer_count;
                self.cursor_builder
                    .merge_cold(bridge_id, &item.touched_blocks);
                return;
            }
        }

        if is_stale {
            self.collect_stale(key, item);
        } else {
            self.collect_hot(key, item);
        }
    }
}

impl<T: Consolidate + Default> MessageBuffer<T> {
    /// Run maintenance: offload stale entries, flush ready entries, update
    /// cursors.
    ///
    /// The maintenance loop performs three logical phases inside a DB
    /// transaction:
    /// 1. **Offload** stale entries to `pending_messages`.
    /// 2. **Flush** consolidatable entries to `crosschain_messages` and
    ///    `crosschain_transfers`.
    /// 3. **Update** `indexer_checkpoints` based on hot/cold cursors.
    ///
    /// After commit, hot entries are removed using CAS to avoid racing with
    /// concurrent updates. Non-final consolidated entries remain in the hot
    /// tier, but their `last_flushed_version` is updated to prevent repeated
    /// upserts until they change.
    ///
    /// Cursor update logic:
    /// - We can only safely advance cursors past blocks where ALL messages have
    ///   been flushed
    /// - Entries still in hot tier or cold storage represent "pending" work
    /// - The realtime_cursor should not advance past the lowest max_block of
    ///   any pending entry
    /// - The catchup_max_cursor should not retreat past the highest min_block of
    ///   any pending entry
    ///
    /// TODO: In case that buffer is full of entries that aren't too old based
    /// on TTL but also not ready yet, we may need to implement a more
    /// aggressive offloading strategy
    pub async fn run(&self) -> Result<()> {
        let _guard = self.maintenance_lock.write().await;
        let maintenance_start = Instant::now();

        let mut plan = self.plan_maintenance()?;

        self.commit_maintenance(&plan).await?;
        self.mark_flushed_versions(&plan.keys_to_mark_flushed);
        self.remove_from_hot_if_unchanged(&plan.hot_evictions, &mut plan.stats);

        let totals = plan.stats.totals();
        tracing::debug!(
            hot_len = self.inner.len(),
            consolidated = plan.consolidated_entries.len(),
            partial = plan.keys_to_mark_flushed.len(),
            stale = totals.stale,
            finalized = totals.finalized_messages,
            not_consolidatable = totals.not_consolidatable,
            removed_stale = totals.removed_stale,
            removed_finalized = totals.removed_finalized,
            skipped = totals.skipped_modified,
            "maintenance completed"
        );

        plan.stats.record_metrics();
        metrics::BUFFER_MAINTENANCE_DURATION.observe(maintenance_start.elapsed().as_secs_f64());
        Ok(())
    }

    fn plan_maintenance(&self) -> Result<MaintenancePlan<T>> {
        let now = Utc::now().naive_utc();
        let mut plan = MaintenancePlan::new();
        for item in self.inner.iter() {
            let key = item.key();
            let value = item.value();
            let age = now
                .signed_duration_since(value.hot_since)
                .max(TimeDelta::zero())
                .to_std()?;
            let is_stale = age >= self.config.hot_ttl;
            let outcome = classify_item(key, value)?;
            plan.collect(*key, value, outcome, is_stale);
        }
        Ok(plan)
    }

    async fn commit_maintenance(&self, plan: &MaintenancePlan<T>) -> Result<()> {
        let consolidated_entries = plan.consolidated_entries.clone();
        let stale_entries = plan.stale_entries.clone();
        let finalized_keys = plan.finalized_keys.clone();
        let cursor_builder = plan.cursor_builder.clone();

        // Widened per coding-task-4b item 1: the stats hook and token
        // enrichment now run for **every** flushed entry, final and `Partial`
        // — a non-final consolidation is already flushed to
        // `crosschain_messages`/`crosschain_transfers`
        // (`ConsolidationOutcome::Partial`), so identity maintenance must see
        // its canonical row too, not only finalized ones. `is_final` stays
        // load-bearing everywhere else below: `finalized_keys` (pending
        // cleanup), `hot_evictions` (eviction), and `BridgeCounts` metrics are
        // all computed from `plan` directly and untouched by this change.
        let flushed_for_stats = consolidated_entries.clone();
        let flushed_for_enrichment = consolidated_entries.clone();

        let stats = self.stats.clone();
        let new = self
            .stats
            .interchain_db()
            .db
            .transaction::<_, Cursors, DbErr>(move |tx| {
                let stats = stats.clone();
                Box::pin(async move {
                    persistence::offload_stale_to_pending(tx, &stale_entries).await?;
                    persistence::flush_to_final_storage(tx, consolidated_entries).await?;
                    stats
                        .apply_stats_for_flushed_batch(tx, &flushed_for_stats)
                        .await?;
                    persistence::remove_finalized_from_pending(tx, &finalized_keys).await?;

                    let old = persistence::fetch_cursors(&cursor_builder, tx).await?;
                    let new = cursor_builder.calculate_updates(&old);
                    tracing::debug!(new =? new, "cursor maintenance");
                    persistence::upsert_cursors(tx, &new).await?;
                    Ok(new)
                })
            })
            .await
            .map_err(anyhow::Error::from)
            .context("maintenance transaction failed")?;

        self.stats
            .kickoff_token_enrichment_for_flushed(&flushed_for_enrichment);

        for ((bridge_id, chain_id), cursor) in &new {
            let bridge_label = bridge_id.to_string();
            let chain_label = chain_id.to_string();
            metrics::BUFFER_CURSOR
                .with_label_values(&[&bridge_label, &chain_label, "catchup"])
                .set(cursor.backward as f64);
            metrics::BUFFER_CURSOR
                .with_label_values(&[&bridge_label, &chain_label, "realtime"])
                .set(cursor.forward as f64);
        }

        Ok(())
    }

    fn mark_flushed_versions(&self, keys_to_mark_flushed: &[(Key, BufferItemVersion)]) {
        keys_to_mark_flushed.iter().for_each(|(key, version)| {
            self.inner.alter(key, |_, item| item.flushed_at(*version));
        });
    }

    fn remove_from_hot_if_unchanged(
        &self,
        keys: &[(Key, BufferItemVersion, HotEvictionReason)],
        stats: &mut BridgeCounts,
    ) {
        for (key, expected_version, reason) in keys {
            let removed = self
                .inner
                .remove_if(key, |_, item| item.version == *expected_version)
                .is_some();
            let bridge_stats = stats.entry(key.bridge_id);
            if removed {
                match reason {
                    HotEvictionReason::Stale => bridge_stats.removed_stale += 1,
                    HotEvictionReason::Finalized => bridge_stats.removed_finalized += 1,
                }
            } else {
                bridge_stats.skipped_modified += 1;
                bridge_stats.hot_entries += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use chrono::Utc;
    use interchain_indexer_entity::{
        bridges, chains, crosschain_messages, crosschain_transfers, pending_messages,
        sea_orm_active_enums::MessageStatus,
    };
    use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter, prelude::BigDecimal};
    use serde::{Deserialize, Serialize};

    use super::{BufferItem, Consolidate, ConsolidatedMessage, Key, MessageBuffer};
    use crate::{
        InterchainDatabase, StatsReadSettings, StatsService, settings::MessageBufferSettings,
        stats::IndexedChains, test_utils::init_db,
    };

    /// Minimal `Consolidate` impl carrying one transfer, used only to drive
    /// `MessageBuffer::run()` end to end (offload/restore/flush/stats hook)
    /// without pulling in a real protocol indexer.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct TransferDummyMessage {
        consolidatable: bool,
        is_final: bool,
    }

    impl Consolidate for TransferDummyMessage {
        fn consolidate(&self, key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
            if !self.consolidatable {
                return Ok(None);
            }
            Ok(Some(ConsolidatedMessage {
                is_final: self.is_final,
                replace_existing: false,
                message: crosschain_messages::ActiveModel {
                    id: ActiveValue::Set(key.message_id),
                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                    status: ActiveValue::Set(MessageStatus::Initiated),
                    init_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                    src_chain_id: ActiveValue::Set(1),
                    dst_chain_id: ActiveValue::Set(Some(100)),
                    src_tx_hash: ActiveValue::Set(Some(vec![0xabu8; 32])),
                    stats_processed: ActiveValue::Set(0),
                    ..Default::default()
                },
                transfers: vec![crosschain_transfers::ActiveModel {
                    message_id: ActiveValue::Set(key.message_id),
                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                    index: ActiveValue::Set(0),
                    token_src_chain_id: ActiveValue::Set(1),
                    token_dst_chain_id: ActiveValue::Set(100),
                    src_amount: ActiveValue::Set(Some(BigDecimal::from(10u64))),
                    dst_amount: ActiveValue::Set(Some(BigDecimal::from(10u64))),
                    token_src_address: ActiveValue::Set(Some(vec![0x11u8; 20])),
                    token_dst_address: ActiveValue::Set(Some(vec![0x22u8; 20])),
                    stats_processed: ActiveValue::Set(0),
                    ..Default::default()
                }],
                amb_confirmations: vec![],
                amb_anomalies: vec![],
            }))
        }
    }

    fn test_buffer_settings() -> MessageBufferSettings {
        MessageBufferSettings {
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        }
    }

    /// coding-task-4b: a `Partial` (non-final) flush must reach the stats
    /// hook and count exactly once; a later finalizing flush of the *same*
    /// canonical key must not recount it. Also exercises the cold-tier path
    /// (offloaded to `pending_messages`, restored via `alter`) end to end
    /// through the real `MessageBuffer::run()` maintenance cycle.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_cold_tier_restore_projects_exactly_once() {
        let test_db = init_db("maintenance_cold_tier_restore_projects_once").await;
        let db = InterchainDatabase::new(test_db.client());

        let key = Key::new(9001, 1);

        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(key.bridge_id as i32),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();
        db.upsert_chains(vec![
            chains::ActiveModel {
                id: ActiveValue::Set(1),
                name: ActiveValue::Set("src".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: ActiveValue::Set(100),
                name: ActiveValue::Set("unindexed_dst".to_string()),
                ..Default::default()
            },
        ])
        .await
        .unwrap();

        // Chain 100 is unindexed for bridge 1, so the `Initiated` (never
        // `Completed`) message/transfer this dummy produces is countable via
        // the "destination confirmation can never arrive" branch — without
        // this, nothing here would ever become countable regardless of
        // `is_final`, and the test would not exercise the widened trigger.
        let stats = Arc::new(StatsService::new(
            Arc::new(db.clone()),
            None,
            StatsReadSettings::default(),
            IndexedChains::from_pairs([(1, 1)]),
        ));
        let buffer =
            MessageBuffer::<TransferDummyMessage>::new_with_stats(stats, test_buffer_settings());

        // Seed the cold tier as if this entry had been offloaded while still
        // NotReady (not yet consolidatable).
        let cold_entry = BufferItem::new(TransferDummyMessage {
            consolidatable: false,
            is_final: false,
        });
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            payload: ActiveValue::Set(serde_json::to_value(&cold_entry).unwrap()),
            created_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        })
        .await
        .unwrap();
        assert!(
            buffer.inner.get(&key).is_none(),
            "must start cold, not in the hot tier"
        );

        // Restore from cold tier (via `alter`) and make it Partial-ready.
        buffer
            .alter(key, 1, 1, |m: &mut TransferDummyMessage| {
                m.consolidatable = true;
                m.is_final = false;
                Ok(())
            })
            .await
            .unwrap();
        assert!(
            buffer.inner.get(&key).is_some(),
            "restore must promote the entry to the hot tier"
        );

        buffer.run().await.unwrap();

        let load_transfer = || {
            crosschain_transfers::Entity::find()
                .filter(crosschain_transfers::Column::MessageId.eq(key.message_id))
                .filter(crosschain_transfers::Column::BridgeId.eq(key.bridge_id as i32))
                .one(db.db.as_ref())
        };
        let t = load_transfer()
            .await
            .unwrap()
            .expect("the Partial flush must have written the transfer row");
        assert_eq!(
            t.stats_processed, 1,
            "a Partial entry must still reach the stats hook and count"
        );
        assert!(t.stats_asset_id.is_some());

        assert!(
            buffer.inner.get(&key).is_some(),
            "a non-final entry stays in the hot tier after maintenance"
        );

        // Finalize and run maintenance again.
        buffer
            .alter(key, 1, 2, |m: &mut TransferDummyMessage| {
                m.is_final = true;
                Ok(())
            })
            .await
            .unwrap();
        buffer.run().await.unwrap();

        let t2 = load_transfer().await.unwrap().unwrap();
        assert_eq!(
            t2.stats_processed, 1,
            "the finalizing flush of the same canonical key must not recount it"
        );
        let msg = crosschain_messages::Entity::find_by_id((key.message_id, key.bridge_id as i32))
            .one(db.db.as_ref())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            msg.stats_processed, 1,
            "the message side must not be recounted either"
        );

        assert!(
            buffer.inner.get(&key).is_none(),
            "the finalized entry must be evicted from the hot tier"
        );
    }
}
