// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{
    collections::{HashMap, HashSet},
    iter::Sum,
    ops::Add,
    time::Instant,
};

use anyhow::{Context, Result};
use chrono::{TimeDelta, Utc};
use sea_orm::{DbErr, TransactionTrait};

use super::{
    BufferItem, BufferItemVersion, Consolidate, ConsolidatedMessage, DestinationExecution,
    DetachedConfirmations, Key, MessageBuffer, persistence,
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
    /// Observed destination-executions from every dirty entry, keyed with the
    /// version seen at planning time so a resolved key can be CAS-evicted
    /// from hot after commit without racing a concurrent mutation. Empty for
    /// every indexer except xDai today: `Consolidate::destination_executions`
    /// defaults to an empty `Vec`, so AMB and Avalanche never populate this
    /// and every operation gated on it is a no-op for them (Hard Constraint 3).
    destination_executions: Vec<(Key, BufferItemVersion, Vec<DestinationExecution>)>,
    /// Confirmations reported by dirty `NotReady` entries through
    /// `Consolidate::detached_confirmations`, with the planning-time version so a
    /// resolved key can be CAS-evicted after commit. Empty for AMB and Avalanche,
    /// which keep the `None` default (Hard Constraint 3).
    detached_confirmations: Vec<(Key, BufferItemVersion, DetachedConfirmations)>,
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

        let resolved_keys = self.commit_maintenance(&plan).await?;
        self.mark_flushed_versions(&plan.keys_to_mark_flushed);
        self.remove_from_hot_if_unchanged(&plan.hot_evictions, &mut plan.stats);
        self.evict_resolved_keys(&plan, &resolved_keys);

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

            // Same dirty gate as `classify_item`'s own `is_dirty()` check:
            // `Unchanged` is exactly the outcome for an item that was not
            // dirty, so anything else implies it was. An already-reconciled,
            // unchanged entry must not re-feed observations into the plan
            // every cycle.
            if !matches!(outcome, ConsolidationOutcome::Unchanged) {
                let observations = value.inner.destination_executions(key);
                if !observations.is_empty() {
                    plan.destination_executions
                        .push((*key, value.version, observations));
                }
            }

            // Called before `plan.collect` consumes `outcome`. `NotReady`
            // implies dirty, and the block runs for hot and stale entries
            // alike: a stale entry is offloaded to `pending_messages` in the
            // same transaction, and the attach step clears that row again
            // only when it resolves the key.
            if matches!(outcome, ConsolidationOutcome::NotReady)
                && let Some(detached) = value.inner.detached_confirmations(key)
            {
                plan.detached_confirmations
                    .push((*key, value.version, detached));
            }

            plan.collect(*key, value, outcome, is_stale);
        }
        Ok(plan)
    }

    /// Returns the keys that a reconciliation channel determined have nothing
    /// left to wait for, so `run()` can evict them from hot after commit. There
    /// are two channels: `reconcile_destination_executions` (destination
    /// executions) and `attach_detached_confirmations` (confirmations of an
    /// entry that cannot consolidate). The result is their union, without
    /// duplicates. It is empty whenever both `plan.destination_executions` and
    /// `plan.detached_confirmations` are empty (AMB, Avalanche, and any xDai
    /// cycle whose dirty entries carry neither) -- both functions return before
    /// any query in that case (Hard Constraint 3).
    async fn commit_maintenance(&self, plan: &MaintenancePlan<T>) -> Result<Vec<Key>> {
        let mut consolidated_entries = plan.consolidated_entries.clone();
        let stale_entries = plan.stale_entries.clone();
        let finalized_keys = plan.finalized_keys.clone();
        let cursor_builder = plan.cursor_builder.clone();
        let observations: Vec<(Key, Vec<DestinationExecution>)> = plan
            .destination_executions
            .iter()
            .map(|(key, _version, observations)| (*key, observations.clone()))
            .collect();
        let detached: Vec<(Key, DetachedConfirmations)> = plan
            .detached_confirmations
            .iter()
            .map(|(key, _version, detached)| (*key, detached.clone()))
            .collect();

        // Widened per coding-task-4b item 1: the stats hook and token
        // enrichment now run for **every** flushed entry, final and `Partial`
        // — a non-final consolidation is already flushed to
        // `crosschain_messages`/`crosschain_transfers`
        // (`ConsolidationOutcome::Partial`), so identity maintenance must see
        // its canonical row too, not only finalized ones. `is_final` stays
        // load-bearing everywhere else below: `finalized_keys` (pending
        // cleanup), `hot_evictions` (eviction), and `BridgeCounts` metrics are
        // all computed from `plan` directly and untouched by this change.
        //
        // Cloned from `consolidated_entries` **before** the destination-
        // execution reconciliation below runs, so neither sees the
        // neutralization `reconcile_destination_executions` may apply to the
        // (separate, moved-into-the-transaction) `consolidated_entries`
        // variable. That is safe today: `apply_stats_for_flushed_batch` only
        // reads each entry's primary key, and
        // `token_keys_from_flushed_for_enrichment` only reads token
        // addresses -- neither field neutralization touches. (When the stored
        // execution wins, reconciliation drops the entry's transfers from the
        // flushed batch; the clones below still carry them, which only means
        // enrichment may fetch metadata for token addresses that xDai derives
        // from the shared source side anyway.) If stats ever
        // starts reading destination fields (`dst_tx_hash`, amounts) off
        // these models, this stops being true and becomes a defect.
        let flushed_for_stats = consolidated_entries.clone();
        let flushed_for_enrichment = consolidated_entries.clone();

        let stats = self.stats.clone();
        let (new, resolved_keys) = self
            .stats
            .interchain_db()
            .db
            .transaction::<_, (Cursors, Vec<Key>), DbErr>(move |tx| {
                let stats = stats.clone();
                Box::pin(async move {
                    persistence::offload_stale_to_pending(tx, &stale_entries).await?;

                    // Reads the stored destination state, decides promotion
                    // and retention per key, and neutralizes
                    // `consolidated_entries`' destination-owned fields when
                    // the stored execution wins (dropping that entry's
                    // transfers) -- all **before** the upsert below, so a
                    // late/non-canonical execution this buffer instance saw
                    // first cannot clobber `dst_tx_hash` /
                    // `recipient_address` or the stored canonical transfer.
                    let reconciliation = persistence::reconcile_destination_executions(
                        tx,
                        &mut consolidated_entries,
                        &observations,
                    )
                    .await?;

                    persistence::flush_to_final_storage(tx, consolidated_entries).await?;

                    // After the flush, so a row this same transaction just
                    // wrote already exists for the anomaly rows/metadata
                    // patch to reference.
                    persistence::apply_destination_execution_reconciliation(tx, &reconciliation)
                        .await?;

                    // After the flush (so a row this same transaction just
                    // wrote exists) and after the stale offload (so a stale
                    // entry's fresh pending row is deleted again below), and
                    // before the pending cleanup that consumes its result.
                    let attached_keys =
                        persistence::attach_detached_confirmations(tx, &detached).await?;

                    stats
                        .apply_stats_for_flushed_batch(tx, &flushed_for_stats)
                        .await?;

                    // Order-preserving union: a key resolved by both channels
                    // is cleared and evicted once.
                    let mut seen: HashSet<Key> = HashSet::new();
                    let resolved_keys: Vec<Key> = reconciliation
                        .resolved_keys
                        .iter()
                        .copied()
                        .chain(attached_keys)
                        .filter(|key| seen.insert(*key))
                        .collect();

                    let mut finalized_keys = finalized_keys;
                    finalized_keys.extend(resolved_keys.iter().copied());
                    persistence::remove_finalized_from_pending(tx, &finalized_keys).await?;

                    let old = persistence::fetch_cursors(&cursor_builder, tx).await?;
                    let new = cursor_builder.calculate_updates(&old);
                    tracing::debug!(new =? new, "cursor maintenance");
                    persistence::upsert_cursors(tx, &new).await?;
                    Ok((new, resolved_keys))
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

        Ok(resolved_keys)
    }

    /// CAS-evicts keys a reconciliation channel reported as resolved (nothing
    /// left to wait for), by the version recorded in the plan at planning time --
    /// the same optimistic-concurrency mechanism `remove_from_hot_if_unchanged`
    /// uses, kept separate from it because that function's bookkeeping
    /// (`removed_*`, `skipped_modified`) is for the `stale`/`finalized` reasons
    /// only. The channels are `reconcile_destination_executions` and
    /// `attach_detached_confirmations`; `resolved_hot_evictions` selects what
    /// this evicts and does not touch `Counts` or metrics.
    ///
    /// Each key is evicted **at most once**, and never a key `plan.hot_evictions`
    /// already covers (the common case: an ordinary, single-execution message is
    /// both finalized and destination-resolved). The reason is not bookkeeping
    /// but ABA: the CAS guards against modification of *one entry instance*, not
    /// against re-creation. After a successful first eviction a concurrent
    /// `alter` can re-create the key, and a fresh default entry restarts at
    /// version 0 and reaches 1 after one mutation. That can equal a late entry's
    /// planning-time version, so a second `remove_if` would evict evidence that
    /// was never persisted.
    ///
    /// A key can also never resolve: a nonce-keyed destination completion
    /// with no stored canonical row and no source facts anywhere stays
    /// `NotReady` forever (`xdai::consolidation::resolve_input`'s
    /// `(None, None)` arm requires `SourceTransactionHash`, which a
    /// nonce-observed message never has), so it stays dirty and
    /// `reconcile_destination_executions` re-queries the database for it on
    /// every cycle until a source eventually arrives. This is expected and
    /// deliberate -- no counter or retry limit is needed for it, the same way
    /// none exists for the standalone (never-executed) hash-keyed
    /// `SignedForAffirmation` confirmations this mirrors.
    fn evict_resolved_keys(&self, plan: &MaintenancePlan<T>, resolved_keys: &[Key]) {
        if resolved_keys.is_empty() {
            return;
        }
        for (key, expected_version) in resolved_hot_evictions(plan, resolved_keys) {
            self.inner
                .remove_if(&key, |_, item| item.version == expected_version);
        }
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

/// `(key, planning-time version)` pairs to CAS-evict after commit because a
/// reconciliation channel resolved them. Each key appears at most once, never
/// a key `plan.hot_evictions` already covers, and only keys with a recorded
/// planning-time version.
fn resolved_hot_evictions<T: Consolidate + Default>(
    plan: &MaintenancePlan<T>,
    resolved_keys: &[Key],
) -> Vec<(Key, BufferItemVersion)> {
    let already_evicted: HashSet<Key> = plan.hot_evictions.iter().map(|(key, _, _)| *key).collect();
    // Both channels capture `value.version` in the same `plan_maintenance`
    // loop iteration, so a key present in both carries the same version.
    let planned_versions: HashMap<Key, BufferItemVersion> = plan
        .destination_executions
        .iter()
        .map(|(key, version, _)| (*key, *version))
        .chain(
            plan.detached_confirmations
                .iter()
                .map(|(key, version, _)| (*key, *version)),
        )
        .collect();

    let mut seen: HashSet<Key> = HashSet::new();
    resolved_keys
        .iter()
        .filter(|key| seen.insert(**key) && !already_evicted.contains(*key))
        .filter_map(|key| planned_versions.get(key).map(|version| (*key, *version)))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use chrono::Utc;
    use interchain_indexer_entity::{
        amb_messages_confirmations, bridges, chains, crosschain_messages, crosschain_transfers,
        indexer_checkpoints, pending_messages,
        sea_orm_active_enums::{MessageStatus, TransferAssetLinkage},
    };
    use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter, prelude::BigDecimal};
    use serde::{Deserialize, Serialize};

    use super::{
        BufferItem, Consolidate, ConsolidatedMessage, DestinationExecution, DetachedConfirmations,
        HotEvictionReason, Key, MaintenancePlan, MessageBuffer, resolved_hot_evictions,
    };
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
                    asset_linkage: ActiveValue::Set(Some(TransferAssetLinkage::Mirror)),
                    ..Default::default()
                }],
                amb_confirmations: vec![],
                amb_anomalies: vec![],
            }))
        }
    }

    /// A message that is destination-only (never consolidatable -- there is
    /// no source side at all) but reports one observed destination-execution,
    /// used to drive the new `destination_executions` channel end to end
    /// through `MessageBuffer::run()` without pulling in xDai.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DestinationOnlyDummyMessage {
        tx_hash: Vec<u8>,
    }

    impl Consolidate for DestinationOnlyDummyMessage {
        fn consolidate(&self, _key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
            // Permanently `NotReady`: this double mirrors a destination-only
            // xDai message with no source facts anywhere.
            Ok(None)
        }

        fn destination_executions(&self, key: &Key) -> Vec<DestinationExecution> {
            if self.tx_hash.is_empty() {
                return Vec::new();
            }
            vec![DestinationExecution {
                key: *key,
                native_id: vec![0xAA; 32],
                chain_id: 100,
                tx_hash: self.tx_hash.clone(),
                log_index: Some(1),
                block_number: 20,
                block_timestamp: Utc::now().naive_utc(),
                executor: Some(vec![0xEE]),
                src_chain_id: Some(1),
                dst_chain_id: Some(100),
                detail: "test destination execution".to_string(),
            }]
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
        assert!(t.src_stats_asset_id.is_some());
        assert!(t.dst_stats_asset_id.is_some());

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

    /// A destination-only entry never becomes `is_final` through
    /// `consolidate()` on its own (there is no source side), but once its
    /// canonical row is already stored -- and this cycle's observation is
    /// exactly that canonical execution replayed -- `reconcile_destination_executions`
    /// reports it resolved and `run()` must evict it from hot via the new,
    /// separate CAS path (`evict_resolved_keys`), not only
    /// the pre-existing `is_final` one.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_destination_only_entry_is_evicted_once_its_canonical_row_is_already_stored() {
        let test_db = init_db("maintenance_destination_only_eviction").await;
        let db = InterchainDatabase::new(test_db.client());

        let key = Key::new(9101, 1);

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
                name: ActiveValue::Set("dst".to_string()),
                ..Default::default()
            },
        ])
        .await
        .unwrap();

        // The canonical row already exists, as if an earlier cycle (or a
        // buffer instance that has since restarted) already flushed and
        // evicted it.
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(MessageStatus::Completed),
            init_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
            src_chain_id: ActiveValue::Set(1),
            dst_chain_id: ActiveValue::Set(Some(100)),
            dst_tx_hash: ActiveValue::Set(Some(vec![0xDD])),
            stats_processed: ActiveValue::Set(0),
            ..Default::default()
        })
        .exec(db.db.as_ref())
        .await
        .unwrap();

        let stats = Arc::new(StatsService::new(
            Arc::new(db.clone()),
            None,
            StatsReadSettings::default(),
            IndexedChains::from_pairs([(1, 1)]),
        ));
        let buffer = MessageBuffer::<DestinationOnlyDummyMessage>::new_with_stats(
            stats,
            test_buffer_settings(),
        );

        buffer
            .alter(key, 1, 20, |m: &mut DestinationOnlyDummyMessage| {
                m.tx_hash = vec![0xDD];
                Ok(())
            })
            .await
            .unwrap();
        assert!(buffer.inner.get(&key).is_some());

        buffer.run().await.unwrap();

        assert!(
            buffer.inner.get(&key).is_none(),
            "a destination-only entry resolved against an already-stored canonical row must be \
             evicted from hot, even though it is never `is_final` on its own"
        );

        // The exact replay must not have produced an anomaly: the tx_hash
        // matches the stored canonical one.
        let anomalies = interchain_indexer_entity::amb_message_anomalies::Entity::find()
            .filter(interchain_indexer_entity::amb_message_anomalies::Column::BridgeId.eq(1))
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BufferKey
                    .eq(key.message_id),
            )
            .all(db.db.as_ref())
            .await
            .unwrap();
        assert!(anomalies.is_empty());
    }

    // --- `Consolidate::detached_confirmations` channel (xdai-lost-confirmations) ---

    /// An entry that reports `validators` as detached confirmations and, when
    /// `consolidatable`, consolidates to a final `Completed` message with no
    /// transfers (so a second key can prove the maintenance transaction
    /// committed). Does not implement `destination_executions`.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DetachedConfirmationsDummyMessage {
        /// Validator byte per confirmation (validator address = [b; 20]).
        validators: Vec<u8>,
        confirmation_only: bool,
        /// When true, `consolidate` returns a final `Completed` message with no
        /// transfers (keys 1/100 chains), so another key can prove the tx committed.
        consolidatable: bool,
    }

    fn dummy_confirmation_models(
        key: &Key,
        validators: &[u8],
    ) -> Vec<amb_messages_confirmations::ActiveModel> {
        validators
            .iter()
            .map(|validator| amb_messages_confirmations::ActiveModel {
                message_id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                validator_address: ActiveValue::Set(vec![*validator; 20]),
                tx_hash: ActiveValue::Set(vec![0xC0; 32]),
                block_number: ActiveValue::Set(20),
                block_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            })
            .collect()
    }

    fn completed_without_transfers(key: &Key) -> ConsolidatedMessage {
        ConsolidatedMessage {
            is_final: true,
            replace_existing: false,
            message: crosschain_messages::ActiveModel {
                id: ActiveValue::Set(key.message_id),
                bridge_id: ActiveValue::Set(key.bridge_id as i32),
                status: ActiveValue::Set(MessageStatus::Completed),
                init_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
                src_chain_id: ActiveValue::Set(1),
                dst_chain_id: ActiveValue::Set(Some(100)),
                stats_processed: ActiveValue::Set(0),
                ..Default::default()
            },
            transfers: vec![],
            amb_confirmations: vec![],
            amb_anomalies: vec![],
        }
    }

    impl Consolidate for DetachedConfirmationsDummyMessage {
        fn consolidate(&self, key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
            Ok(self
                .consolidatable
                .then(|| completed_without_transfers(key)))
        }

        fn detached_confirmations(&self, key: &Key) -> Option<DetachedConfirmations> {
            if self.validators.is_empty() {
                return None;
            }
            Some(DetachedConfirmations {
                confirmations: dummy_confirmation_models(key, &self.validators),
                confirmation_only: self.confirmation_only,
            })
        }
    }

    /// Reports both channels for the same key: one observed destination
    /// execution *and* confirmation-only detached confirmations. Artificial on
    /// purpose, to exercise the union and dedupe of the two resolved-key
    /// channels.
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DualChannelDummyMessage {
        tx_hash: Vec<u8>,
        validators: Vec<u8>,
    }

    impl Consolidate for DualChannelDummyMessage {
        fn consolidate(&self, _key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
            Ok(None)
        }

        fn destination_executions(&self, key: &Key) -> Vec<DestinationExecution> {
            if self.tx_hash.is_empty() {
                return Vec::new();
            }
            vec![DestinationExecution {
                key: *key,
                native_id: vec![0xAA; 32],
                chain_id: 100,
                tx_hash: self.tx_hash.clone(),
                log_index: Some(1),
                block_number: 20,
                block_timestamp: Utc::now().naive_utc(),
                executor: Some(vec![0xEE]),
                src_chain_id: Some(1),
                dst_chain_id: Some(100),
                detail: "test destination execution".to_string(),
            }]
        }

        fn detached_confirmations(&self, key: &Key) -> Option<DetachedConfirmations> {
            if self.validators.is_empty() {
                return None;
            }
            Some(DetachedConfirmations {
                confirmations: dummy_confirmation_models(key, &self.validators),
                confirmation_only: true,
            })
        }
    }

    fn buffer_settings_with_ttl(hot_ttl: Duration) -> MessageBufferSettings {
        MessageBufferSettings {
            hot_ttl,
            maintenance_interval: Duration::from_secs(60),
        }
    }

    fn stats_service(db: &InterchainDatabase) -> Arc<StatsService> {
        Arc::new(StatsService::new(
            Arc::new(db.clone()),
            None,
            StatsReadSettings::default(),
            IndexedChains::from_pairs([(1, 1)]),
        ))
    }

    /// Bridge 1 and chains 1 / 100.
    async fn seed_bridge_one_and_chains(db: &InterchainDatabase) {
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(1),
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
                name: ActiveValue::Set("dst".to_string()),
                ..Default::default()
            },
        ])
        .await
        .unwrap();
    }

    /// The canonical row of `key`, as if an earlier cycle already flushed it.
    async fn insert_stored_message(db: &InterchainDatabase, key: Key) {
        crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
            id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            status: ActiveValue::Set(MessageStatus::Completed),
            init_timestamp: ActiveValue::Set(Utc::now().naive_utc()),
            src_chain_id: ActiveValue::Set(1),
            dst_chain_id: ActiveValue::Set(Some(100)),
            dst_tx_hash: ActiveValue::Set(Some(vec![0xDD])),
            stats_processed: ActiveValue::Set(0),
            ..Default::default()
        })
        .exec(db.db.as_ref())
        .await
        .unwrap();
    }

    /// A `pending_messages` row for `key` holding an empty entry of type `T`,
    /// so `alter` restores from the cold tier.
    async fn insert_pending_entry<T: Consolidate + Default>(db: &InterchainDatabase, key: Key) {
        let entry = BufferItem::new(T::default());
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            payload: ActiveValue::Set(serde_json::to_value(&entry).unwrap()),
            created_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        })
        .await
        .unwrap();
    }

    async fn confirmation_count(db: &InterchainDatabase, key: Key) -> usize {
        amb_messages_confirmations::Entity::find()
            .filter(amb_messages_confirmations::Column::MessageId.eq(key.message_id))
            .filter(amb_messages_confirmations::Column::BridgeId.eq(key.bridge_id as i32))
            .all(db.db.as_ref())
            .await
            .unwrap()
            .len()
    }

    async fn pending_row_exists(db: &InterchainDatabase, key: Key) -> bool {
        db.get_pending_message(key.message_id, key.bridge_id as i32)
            .await
            .unwrap()
            .is_some()
    }

    #[test]
    fn resolved_hot_evictions_skips_already_evicted_and_dedupes() {
        let (k1, k2, k3, k4) = (
            Key::new(1, 1),
            Key::new(2, 1),
            Key::new(3, 1),
            Key::new(4, 1),
        );
        let observation = DestinationExecution {
            key: k2,
            native_id: vec![0xAA; 32],
            chain_id: 100,
            tx_hash: vec![0xDD],
            log_index: None,
            block_number: 20,
            block_timestamp: Utc::now().naive_utc(),
            executor: None,
            src_chain_id: None,
            dst_chain_id: None,
            detail: "test".to_string(),
        };
        let detached = || DetachedConfirmations {
            confirmations: vec![],
            confirmation_only: true,
        };
        let plan = MaintenancePlan::<TransferDummyMessage> {
            hot_evictions: vec![(k1, 3, HotEvictionReason::Stale)],
            destination_executions: vec![(k2, 5, vec![observation])],
            detached_confirmations: vec![
                (k1, 3, detached()),
                (k2, 5, detached()),
                (k3, 7, detached()),
            ],
            ..Default::default()
        };

        // k1 is already covered by `hot_evictions` although it also has a
        // planned version, so only the at-most-once guard keeps it out; k2 is
        // resolved twice and planned by both channels, k3 only by the detached
        // channel, and k4 has no planned version at all.
        let resolved = [k1, k2, k2, k3, k4];

        assert_eq!(
            resolved_hot_evictions(&plan, &resolved),
            vec![(k2, 5), (k3, 7)]
        );
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_detached_confirmation_only_entry_with_stored_row_is_attached_and_evicted() {
        let test_db = init_db("maintenance_detached_only_attached_and_evicted").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_one_and_chains(&db).await;
        let key = Key::new(9201, 1);
        insert_stored_message(&db, key).await;
        insert_pending_entry::<DetachedConfirmationsDummyMessage>(&db, key).await;
        let buffer = MessageBuffer::<DetachedConfirmationsDummyMessage>::new_with_stats(
            stats_service(&db),
            test_buffer_settings(),
        );

        buffer
            .alter(key, 1, 20, |m: &mut DetachedConfirmationsDummyMessage| {
                m.validators = vec![1, 2];
                m.confirmation_only = true;
                Ok(())
            })
            .await
            .unwrap();
        buffer.run().await.unwrap();

        assert_eq!(confirmation_count(&db, key).await, 2);
        assert!(
            buffer.inner.get(&key).is_none(),
            "a confirmation-only entry whose message is stored has nothing left to wait for"
        );
        assert!(!pending_row_exists(&db, key).await);
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_detached_confirmations_with_other_evidence_are_attached_but_entry_is_kept() {
        let test_db = init_db("maintenance_detached_other_evidence_kept").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_one_and_chains(&db).await;
        let key = Key::new(9202, 1);
        insert_stored_message(&db, key).await;
        insert_pending_entry::<DetachedConfirmationsDummyMessage>(&db, key).await;
        let buffer = MessageBuffer::<DetachedConfirmationsDummyMessage>::new_with_stats(
            stats_service(&db),
            test_buffer_settings(),
        );

        buffer
            .alter(key, 1, 20, |m: &mut DetachedConfirmationsDummyMessage| {
                m.validators = vec![1, 2];
                m.confirmation_only = false;
                Ok(())
            })
            .await
            .unwrap();
        buffer.run().await.unwrap();

        assert_eq!(confirmation_count(&db, key).await, 2);
        assert!(
            buffer.inner.get(&key).is_some(),
            "an entry with other evidence is still waiting for something else"
        );
        assert!(pending_row_exists(&db, key).await);
    }

    /// A confirmation-only entry without a stored message must neither write
    /// rows (the FK would reject them) nor abort the transaction: an unrelated
    /// key in the same cycle still commits.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_detached_confirmations_without_stored_row_are_neither_written_nor_evicted() {
        let test_db = init_db("maintenance_detached_without_stored_row").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_one_and_chains(&db).await;
        let key_a = Key::new(9203, 1);
        let key_b = Key::new(9204, 1);
        let buffer = MessageBuffer::<DetachedConfirmationsDummyMessage>::new_with_stats(
            stats_service(&db),
            test_buffer_settings(),
        );

        buffer
            .alter(key_a, 1, 30, |m: &mut DetachedConfirmationsDummyMessage| {
                m.consolidatable = true;
                Ok(())
            })
            .await
            .unwrap();
        buffer
            .alter(key_b, 1, 20, |m: &mut DetachedConfirmationsDummyMessage| {
                m.validators = vec![1, 2];
                m.confirmation_only = true;
                Ok(())
            })
            .await
            .unwrap();

        buffer.run().await.expect("the maintenance run must commit");

        assert!(
            crosschain_messages::Entity::find_by_id((key_a.message_id, 1))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .is_some(),
            "the unrelated key's row proves the transaction committed"
        );
        assert!(
            !indexer_checkpoints::Entity::find()
                .filter(indexer_checkpoints::Column::BridgeId.eq(1))
                .all(db.db.as_ref())
                .await
                .unwrap()
                .is_empty(),
            "cursors are written in the same transaction"
        );
        assert!(
            crosschain_messages::Entity::find_by_id((key_b.message_id, 1))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(confirmation_count(&db, key_b).await, 0);
        assert!(
            buffer.inner.get(&key_b).is_some(),
            "the entry keeps waiting for its message"
        );
    }

    async fn resolved_by_both_channels_case(db_name: &str, hot_ttl: Duration) {
        let test_db = init_db(db_name).await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_one_and_chains(&db).await;
        let key = Key::new(9205, 1);
        // The stored canonical row has the very `dst_tx_hash` the observation
        // reports, so the observation is a replay, not an anomaly.
        insert_stored_message(&db, key).await;
        let buffer = MessageBuffer::<DualChannelDummyMessage>::new_with_stats(
            stats_service(&db),
            buffer_settings_with_ttl(hot_ttl),
        );

        buffer
            .alter(key, 1, 20, |m: &mut DualChannelDummyMessage| {
                m.tx_hash = vec![0xDD];
                m.validators = vec![1, 2];
                Ok(())
            })
            .await
            .unwrap();
        buffer.run().await.unwrap();

        assert!(buffer.inner.get(&key).is_none(), "evicted from hot");
        assert_eq!(confirmation_count(&db, key).await, 2);
        assert!(
            !pending_row_exists(&db, key).await,
            "no pending row is left behind"
        );
        let anomalies = interchain_indexer_entity::amb_message_anomalies::Entity::find()
            .filter(interchain_indexer_entity::amb_message_anomalies::Column::BridgeId.eq(1))
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BufferKey
                    .eq(key.message_id),
            )
            .all(db.db.as_ref())
            .await
            .unwrap();
        assert!(anomalies.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_key_resolved_by_both_channels_is_evicted_once() {
        resolved_by_both_channels_case(
            "maintenance_resolved_by_both_channels",
            Duration::from_secs(60),
        )
        .await;
        // With a zero TTL the key is also a Stale eviction: the resolved-key
        // path must skip it, and the end state is the same.
        resolved_by_both_channels_case(
            "maintenance_resolved_by_both_channels_stale",
            Duration::ZERO,
        )
        .await;
    }

    /// A double that keeps the default `detached_confirmations` hook is
    /// neither evicted nor given rows, even with a stored row for its key.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn test_non_participating_not_ready_entry_with_stored_row_is_untouched() {
        let test_db = init_db("maintenance_non_participating_untouched").await;
        let db = InterchainDatabase::new(test_db.client());
        seed_bridge_one_and_chains(&db).await;
        let key = Key::new(9206, 1);
        insert_stored_message(&db, key).await;
        insert_pending_entry::<TransferDummyMessage>(&db, key).await;
        let buffer = MessageBuffer::<TransferDummyMessage>::new_with_stats(
            stats_service(&db),
            test_buffer_settings(),
        );

        buffer
            .alter(key, 1, 20, |m: &mut TransferDummyMessage| {
                m.consolidatable = false;
                Ok(())
            })
            .await
            .unwrap();
        buffer.run().await.unwrap();

        assert!(buffer.inner.get(&key).is_some(), "still hot");
        assert!(pending_row_exists(&db, key).await, "pending row intact");
        assert_eq!(confirmation_count(&db, key).await, 0);
    }
}
