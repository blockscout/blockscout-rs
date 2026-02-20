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
        tracing::info!(
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

        let new = self
            .db
            .db
            .transaction::<_, Cursors, DbErr>(move |tx| {
                Box::pin(async move {
                    persistence::offload_stale_to_pending(tx, &stale_entries).await?;
                    persistence::flush_to_final_storage(tx, consolidated_entries).await?;
                    persistence::remove_finalized_from_pending(tx, &finalized_keys).await?;

                    let old = persistence::fetch_cursors(&cursor_builder, tx).await?;
                    let new = cursor_builder.calculate_updates(&old);
                    persistence::upsert_cursors(tx, &new).await?;
                    Ok(new)
                })
            })
            .await
            .map_err(anyhow::Error::from)
            .context("maintenance transaction failed")?;

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
