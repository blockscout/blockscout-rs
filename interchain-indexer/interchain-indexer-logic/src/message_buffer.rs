//! Tiered message buffer for cross-chain message indexing.
//!
//! # Why this exists
//! Indexing cross-chain messages is inherently *eventual* and *out-of-order*. A
//! single message is observed across multiple chains, multiple transactions,
//! and potentially multiple retries. We cannot write a final record as soon as
//! we see the first event, yet we also cannot keep everything in memory
//! indefinitely. This buffer provides a **durable, concurrent, and
//! incremental** staging area until messages are ready to be persisted.
//!
//! # Two-tier storage
//! - **Hot tier (memory, `DashMap`)**
//!   - Fast mutation during log processing.
//!   - Holds the latest working state for each message.
//!   - Entries are evicted by TTL; this is a performance knob, not a durability
//!     boundary.
//! - **Cold tier (Postgres, `pending_messages`)**
//!   - Durable staging for entries evicted from hot tier.
//!   - On access, entries can be re-hydrated into hot tier.
//!
//! # Promotion to final storage
//! Each entry is periodically **consolidated** into one or more rows in:
//! - `crosschain_messages` (the main message record)
//! - `crosschain_transfers` (optional ICTT transfer record)
//!
//! The consolidation rules are implemented by the indexer via the
//! [`Consolidate`] trait.
//!
//! > Example: In the Avalanche indexer, a message becomes *consolidatable*
//! > after the `SendCrossChainMessage` event is known and becomes *final* only
//! > after execution succeeds **and** any ICTT transfer is complete (see
//! > `indexer/avalanche/consolidation.rs`).
//!
//! # Cursor semantics
//! Each entry tracks, per chain, the min/max block numbers that contributed to
//! it. During maintenance we update `indexer_checkpoints` so the indexer can
//! advance safely:
//! - The **realtime cursor** never advances past the lowest `max_block` of
//!   pending entries.
//! - The **catchup max** never retreats past the highest `min_block` of pending
//!   entries. This is intentionally conservative and may re-process some
//!   blocks; callers must be idempotent. This behavior is a long-term
//!   correctness guarantee.
//!
//! # Concurrency model
//! - Hot tier mutations are per-key and concurrent (via `DashMap`).
//! - Maintenance uses a coarse `RwLock` to serialize DB flushes.
//! - Hot-tier removals are **CAS-protected** by a version counter, preventing
//!   deletion of entries that were updated concurrently.
//!
//! # Usage Example
//! In the Avalanche indexer:
//! - `MessageBuffer::alter` is invoked for every Teleporter/ICTT log to
//!   incrementally update message state (`send`, `receive`, `execution`,
//!   `transfer`).
//! - The background task (`MessageBuffer::start`) periodically calls `run()` to
//!   flush/persist and update cursors.
//!
//! # Schema
//! Mermaid diagram is accessible at
//! https://mermaid.ai/d/063e4ac1-5682-40ab-b2b4-d0b13c925583 or
//! https://gist.github.com/fedor-ivn/d6748d6cc7f0e21c71f9d90856bf6c64
//!
//! # Future improvements    
//! - Migrate from using DashMap to `moka::future::Cache`.

use std::{collections::HashMap, sync::Arc, time::Duration};

use alloy::primitives::{BlockNumber, ChainId};
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, TimeDelta, Utc};
use dashmap::{
    DashMap,
    mapref::{entry::Entry as DashEntry, one::RefMut},
};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, indexer_checkpoints, pending_messages,
};
use itertools::Itertools;
use sea_orm::{
    ActiveValue, DatabaseTransaction, DbErr, EntityTrait, Iterable, QueryFilter, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};

use crate::{
    InterchainDatabase,
    cursor::{BridgeId, Cursor, CursorBlocksBuilder, Cursors},
    metrics,
};

/// Key for identifying a cross-chain message within a specific bridge.
///
/// The caller defines how `message_id` is derived from chain-specific fields.
/// Example: the Avalanche indexer maps Teleporter `messageID` to a compact
/// `i64` (first 8 bytes, big-endian) and combines it with `bridge_id`.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Key {
    pub message_id: i64,
    pub bridge_id: BridgeId,
}

impl Key {
    pub fn new(message_id: i64, bridge_id: BridgeId) -> Self {
        Self {
            message_id,
            bridge_id,
        }
    }
}

/// Result of consolidating a working entry into final storage models.
///
/// `is_final` controls whether the entry can be removed from both tiers after
/// a successful flush.
#[derive(Clone, Debug)]
pub struct ConsolidatedMessage {
    pub is_final: bool,
    pub message: crosschain_messages::ActiveModel,
    pub transfers: Vec<crosschain_transfers::ActiveModel>,
}

/// Converts an in-flight entry into a consolidated database payload.
///
/// Returning:
/// - `Ok(None)` means the entry is *not yet consolidatable* (e.g. missing
///   the required source-side event). The buffer will keep it in hot/cold
///   storage and try again later.
/// - `Ok(Some(..))` yields the models to upsert into final tables.
///
/// The implementation decides when an entry becomes *final*.
pub trait Consolidate:
    Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>
{
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: for<'a> Deserialize<'a>"))]
pub struct BufferItem<T> {
    pub inner: T,
    /// chain_id -> (min_block, max_block) observed for this entry.
    ///
    /// These cursors are used to compute conservative `indexer_checkpoints`
    /// updates during maintenance.
    touched_blocks: HashMap<ChainId, Vec<BlockNumber>>,
    /// Monotonic version used to detect concurrent modifications.
    version: u16,
    /// Last `version` that was successfully flushed into `crosschain_messages`.
    ///
    /// This is used to avoid repeatedly upserting "consolidated but not final"
    /// entries when they haven't changed.
    #[serde(default)]
    last_flushed_version: u16,
    /// Hot-tier insertion timestamp used for TTL-based eviction.
    ///
    /// This field is intentionally not persisted in cold storage payloads;
    /// when an entry is restored from cold tier it will be set to `Utc::now()`.
    #[serde(skip_serializing, default = "now_naive_utc")]
    pub hot_since: NaiveDateTime,
}

fn now_naive_utc() -> NaiveDateTime {
    Utc::now().naive_utc()
}

impl<T: Consolidate> BufferItem<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            touched_blocks: HashMap::new(),
            version: 0,
            last_flushed_version: 0,
            hot_since: now_naive_utc(),
        }
    }

    /// Record that data from a specific block was added to this message.
    /// Updates both min and max to properly track bidirectional cursor movement.
    fn record_block(&mut self, chain_id: ChainId, block_number: BlockNumber) -> &Self {
        self.touched_blocks
            .entry(chain_id)
            .and_modify(|set| {
                set.push(block_number);
            })
            .or_insert(vec![block_number]);
        self
    }

    /// Increment version to indicate modification.
    fn touch(&mut self) -> Result<&Self> {
        self.version = self
            .version
            .checked_add(1)
            .context("version overflow in message buffer entry")?;
        Ok(self)
    }

    fn flushed_at(mut self, version: u16) -> Self {
        self.last_flushed_version = version;
        self
    }

    /// Returns true if this entry has changed since the last successful flush.
    fn is_dirty(&self) -> bool {
        self.version > self.last_flushed_version
    }
}

/// Configuration for the tiered message buffer.
///
/// `hot_ttl` controls how long entries may remain only in memory before they
/// are offloaded to cold storage. It is a performance knob, not a durability
/// boundary. `maintenance_interval` controls how often maintenance runs (flush
/// + offload + cursor update).
#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum entries in hot tier before forced offload
    pub _max_hot_entries: usize,
    /// Time before entry moves from hot to cold
    pub hot_ttl: Duration,
    /// How often to run maintenance (offload + flush)
    pub maintenance_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            _max_hot_entries: 100_000,
            hot_ttl: Duration::from_secs(10),
            maintenance_interval: Duration::from_millis(500),
        }
    }
}

/// Tiered message buffer with hot (memory) and cold (Postgres) storage.
///
/// This struct is safe to share across tasks; per-message updates are
/// synchronized by the underlying `DashMap`.
pub struct MessageBuffer<T: Consolidate> {
    /// Hot tier: in-memory entries
    inner: DashMap<Key, BufferItem<T>>,
    /// Configuration
    config: Config,
    /// Database connection for cold tier operations
    db: InterchainDatabase,
    /// Lock for maintenance operations (offload/flush)
    maintenance_lock: RwLock<()>,
}

impl<T: Consolidate> MessageBuffer<T> {
    /// Create a new tiered message buffer.
    ///
    /// Note: no data is eagerly loaded from cold storage. Entries are restored
    /// on-demand via `get_mut_or_default` / `alter`.
    pub fn new(db: InterchainDatabase, config: Config) -> Arc<Self> {
        Arc::new(Self {
            inner: DashMap::new(),
            config,
            db,
            maintenance_lock: RwLock::new(()),
        })
    }

    /// Get the number of entries in the hot tier (for testing/monitoring)
    ///
    #[allow(dead_code)]
    pub fn hot_len(&self) -> usize {
        self.inner.len()
    }

    async fn restore(&self, key: Key) -> Result<Option<BufferItem<T>>> {
        let row = self
            .db
            .get_pending_message(key.message_id, key.bridge_id as i32)
            .await?;

        let bridge = key.bridge_id.to_string();
        let entry = if let Some(row) = row {
            metrics::BUFFER_RESTORE_TOTAL
                .with_label_values(&[&bridge, "hit"])
                .inc();
            Some(serde_json::from_value(row.payload)?)
        } else {
            metrics::BUFFER_RESTORE_TOTAL
                .with_label_values(&[&bridge, "miss"])
                .inc();
            None
        };

        Ok(entry)
    }

    /// Get existing entry.
    ///
    /// Checks:
    /// 1. hot tier
    /// 2. cold tier (pending_messages)
    pub async fn _get_mut(&self, key: Key) -> Result<Option<RefMut<'_, Key, BufferItem<T>>>> {
        match self.inner.get_mut(&key) {
            Some(value) => Ok(Some(value)),
            None => {
                let entry = self
                    .restore(key)
                    .await?
                    .map(|value| match self.inner.entry(key) {
                        DashEntry::Occupied(entry) => entry.into_ref(),
                        DashEntry::Vacant(entry) => entry.insert(value),
                    });
                Ok(entry)
            }
        }
    }

    /// Get existing entry or insert a new default entry, returning a mutable reference.
    ///
    /// If the entry is restored from cold tier, its `hot_since` is reset to
    /// `Utc::now()` to ensure a full TTL in memory.
    pub async fn get_mut_or_default(&self, key: Key) -> Result<RefMut<'_, Key, BufferItem<T>>>
    where
        T: Default,
    {
        match self.inner.get_mut(&key) {
            Some(entry) => Ok(entry),
            None => {
                let value = self
                    .restore(key)
                    .await?
                    .unwrap_or_else(|| BufferItem::new(T::default()));

                let entry = match self.inner.entry(key) {
                    DashEntry::Occupied(entry) => entry.into_ref(),
                    DashEntry::Vacant(entry) => entry.insert(value),
                };

                Ok(entry)
            }
        }
    }

    /// TODO: check once again if this is needed
    /// If hot tier exceeds capacity, runs maintenance to flush entries.
    async fn _maybe_run(&self) -> Result<()> {
        if self.inner.len() > self.config._max_hot_entries {
            self.run().await?;
        }
        Ok(())
    }

    /// Get-or-create the entry, mutate its inner message, and record cursors.
    ///
    /// This is the primary API used by indexers during log processing. It
    /// ensures that every mutation increments the entry `version` and records
    /// the chain/block that produced the update (for safe cursor advancement).
    pub async fn alter(
        &self,
        key: Key,
        chain_id: ChainId,
        block_number: BlockNumber,
        mutator: impl FnOnce(&mut T) -> Result<()>,
    ) -> Result<()>
    where
        T: Default,
    {
        let mut entry = self.get_mut_or_default(key).await?;
        mutator(&mut entry.inner)?;
        entry.record_block(chain_id, block_number);
        entry.touch()?;
        Ok(())
    }

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
        let now = Utc::now().naive_utc();

        // Collect entries to process
        let mut consolidated_entries = Vec::new();
        let mut stale_entries = Vec::new();
        // Store (key, version) pairs for CAS removal after DB commit
        let mut keys_to_remove_from_hot: Vec<(Key, u16)> = Vec::new();
        let mut keys_to_remove_from_pending = Vec::new();
        let mut cursor_builder = CursorBlocksBuilder::new();

        // Track which keys were flushed so we can update `last_flushed_version` after commit.
        // (We only need this for non-final entries that remain in hot tier.)
        let mut keys_to_flush: Vec<(Key, u16)> = Vec::new();

        // Diagnostics: why entries are not being removed
        let mut not_consolidatable_count = 0usize;
        let mut consolidated_not_final_count = 0usize;
        let mut final_count = 0usize;

        for entry in self.inner.iter() {
            let key = *entry.key();
            let entry_version = entry.version;
            let created_at = entry.hot_since;

            let age = now
                .signed_duration_since(created_at)
                .max(TimeDelta::zero())
                .to_std()?;
            let is_stale = age >= self.config.hot_ttl;

            let is_dirty = entry.is_dirty();
            let consolidated = if is_dirty {
                entry.inner.consolidate(&key)?
            } else {
                None
            };

            let is_final = if let Some(message) = consolidated {
                let is_final = message.is_final;
                if is_final {
                    final_count += 1;
                    keys_to_remove_from_pending.push(key);
                } else {
                    consolidated_not_final_count += 1;
                    keys_to_flush.push((key, entry_version));
                }
                consolidated_entries.push(message);
                is_final
            } else {
                if is_dirty {
                    not_consolidatable_count += 1;
                }
                false
            };

            let should_remove = is_final || is_stale;

            if should_remove {
                if !is_final {
                    stale_entries.push((key, entry.clone()));
                }
                keys_to_remove_from_hot.push((key, entry_version));
                cursor_builder.merge_cold(key.bridge_id, &entry.touched_blocks);
            } else {
                cursor_builder.merge_hot(key.bridge_id, &entry.touched_blocks);
            }
        }

        let ready_count = consolidated_entries.len();
        let stale_count = stale_entries.len();

        let cursors = self
            .db
            .db
            .transaction::<_, Cursors, DbErr>(|tx| {
                let cursor_builder = cursor_builder.clone();
                Box::pin(async move {
                    let batch_size =
                        (u16::MAX - 100) as usize / pending_messages::Column::iter().count();
                    for batch in stale_entries.chunks(batch_size) {
                        let models: Vec<pending_messages::ActiveModel> = batch
                            .iter()
                            .map(|(key, entry)| {
                                let payload = serde_json::to_value(entry).map_err(|e| {
                                    DbErr::Custom(format!(
                                        "pending_messages payload serialize failed: {e}"
                                    ))
                                })?;
                                Ok(pending_messages::ActiveModel {
                                    message_id: ActiveValue::Set(key.message_id),
                                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                                    payload: ActiveValue::Set(payload),
                                    created_at: ActiveValue::Set(Some(entry.hot_since)),
                                })
                            })
                            .collect::<Result<_, DbErr>>()?;

                        pending_messages::Entity::insert_many(models)
                            .on_conflict(
                                OnConflict::columns([
                                    pending_messages::Column::MessageId,
                                    pending_messages::Column::BridgeId,
                                ])
                                .update_column(pending_messages::Column::Payload)
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    // 2. Flush ready entries to final storage
                    let batch_size =
                        (u16::MAX - 100) as usize / crosschain_messages::Column::iter().count();
                    let (messages, transfers): (Vec<_>, Vec<_>) = consolidated_entries
                        .into_iter()
                        .map(|c| (c.message, c.transfers))
                        .unzip();
                    let transfers = transfers.into_iter().flatten().collect::<Vec<_>>();

                    for batch in messages.chunks(batch_size) {
                        crosschain_messages::Entity::insert_many(batch.to_vec())
                            .on_conflict(
                                OnConflict::columns([
                                    crosschain_messages::Column::Id,
                                    crosschain_messages::Column::BridgeId,
                                ])
                                .update_columns([
                                    crosschain_messages::Column::Status,
                                    crosschain_messages::Column::DstChainId,
                                    crosschain_messages::Column::DstTxHash,
                                    crosschain_messages::Column::LastUpdateTimestamp,
                                    crosschain_messages::Column::SenderAddress,
                                    crosschain_messages::Column::RecipientAddress,
                                    crosschain_messages::Column::Payload,
                                ])
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    let batch_size =
                        (u16::MAX - 100) as usize / crosschain_transfers::Column::iter().count();
                    for batch in transfers.chunks(batch_size) {
                        crosschain_transfers::Entity::insert_many(batch.to_vec())
                            .on_conflict(
                                OnConflict::columns([
                                    crosschain_transfers::Column::MessageId,
                                    crosschain_transfers::Column::BridgeId,
                                    crosschain_transfers::Column::Index,
                                ])
                                .update_columns(crosschain_transfers::Column::iter().filter(|c| {
                                    !matches!(
                                        c,
                                        crosschain_transfers::Column::Id
                                            | crosschain_transfers::Column::MessageId
                                            | crosschain_transfers::Column::BridgeId
                                            | crosschain_transfers::Column::Index
                                            | crosschain_transfers::Column::CreatedAt
                                    )
                                }))
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    let batch_size =
                        (u16::MAX - 100) as usize / pending_messages::Column::iter().count();
                    for batch in keys_to_remove_from_pending.chunks(batch_size) {
                        let batch = batch.iter().map(|k| (k.message_id, k.bridge_id));

                        pending_messages::Entity::delete_many()
                            .filter(
                                Expr::tuple([
                                    Expr::col(pending_messages::Column::MessageId).into(),
                                    Expr::col(pending_messages::Column::BridgeId).into(),
                                ])
                                .in_tuples(batch),
                            )
                            .exec(tx)
                            .await?;
                    }

                    let existing = fetch_existing_cursors(&cursor_builder, tx).await?;
                    let cursors = cursor_builder.calculate_updates(&existing);
                    update_indexer_cursors(tx, &cursors).await?;
                    Ok(cursors)
                })
            })
            .await
            .map_err(anyhow::Error::from)
            .context("maintenance transaction failed")?;

        keys_to_flush.iter().for_each(|(key, version)| {
            self.inner.alter(key, |_, entry| entry.flushed_at(*version));
        });

        // Remove processed entries from hot tier using CAS (Compare-And-Swap).
        // Only remove if the version hasn't changed since we captured it during iteration.
        // If version changed, the entry was modified concurrently and will be processed
        // in the next maintenance cycle.
        let keys_with_outcomes_per_bridge = keys_to_remove_from_hot
            .iter()
            .map(|(key, expected_version)| {
                let is_removed = self
                    .inner
                    .remove_if(key, |_, entry| entry.version == *expected_version)
                    .is_some();
                (key, is_removed)
            })
            .into_group_map_by(|(key, _)| key.bridge_id);

        let flushed_keys_per_bridge = keys_with_outcomes_per_bridge
            .iter()
            .map(|(bridge_id, outcomes)| {
                let (removed, skipped): (Vec<bool>, Vec<bool>) = outcomes
                    .iter()
                    .map(|(_, is_removed)| *is_removed)
                    .partition(|is_removed| *is_removed);
                (bridge_id, (removed.len(), skipped.len()))
            })
            .collect::<HashMap<_, _>>();

        flushed_keys_per_bridge
            .iter()
            .for_each(|(bridge_id, (removed_count, _))| {
                let bridge_label = bridge_id.to_string();
                metrics::BUFFER_FLUSH_FINAL_ENTRIES
                    .with_label_values(&[bridge_label.as_str()])
                    .observe(*removed_count as f64);
            });

        let (removed_count, skipped_count) = flushed_keys_per_bridge.values().fold(
            (0usize, 0usize),
            |(acc_removed, acc_skipped), (removed, skipped)| {
                (acc_removed + removed, acc_skipped + skipped)
            },
        );

        tracing::info!(
            ready = ready_count,
            stale = stale_count,
            cursors = cursors.len(),
            removed = removed_count,
            skipped = skipped_count,
            hot_len = self.inner.len(),
            not_consolidatable = not_consolidatable_count,
            consolidated_not_final = consolidated_not_final_count,
            r#final = final_count,
            "maintenance completed"
        );

        self.inner
            .iter()
            .counts_by(|entry| entry.key().bridge_id)
            .iter()
            .for_each(|(bridge_id, count)| {
                let bridge_label = bridge_id.to_string();
                metrics::BUFFER_HOT_ENTRIES
                    .with_label_values(&[bridge_label.as_str()])
                    .set(*count as f64);
            });

        cursors.iter().for_each(|((bridge_id, chain_id), cursor)| {
            let bridge_label = bridge_id.to_string();
            let chain_label = chain_id.to_string();
            metrics::BUFFER_CATCHUP_CURSOR
                .with_label_values(&[&bridge_label, &chain_label])
                .set(cursor.backward as f64);
            metrics::BUFFER_REALTIME_CURSOR
                .with_label_values(&[&bridge_label, &chain_label])
                .set(cursor.forward as f64);
        });

        Ok(())
    }

    /// Start the background maintenance loop.
    ///
    /// ~~Restores pending messages from cold storage and spawns a maintenance
    /// task.~~
    ///
    /// TODO: do we actually need to restore pending messages into hot tier? It
    /// may be better to just leave them in cold storage and load on-demand.
    pub async fn start(self: Arc<Self>) -> Result<JoinHandle<()>> {
        // let count = pending_messages::Entity::find()
        //     .order_by_asc(pending_messages::Column::CreatedAt)
        //     .limit(self.config.max_hot_entries as u64)
        //     .all(self.db.db.as_ref())
        //     .await
        //     .context("failed to load pending messages from cold storage")?
        //     .iter()
        //     .try_fold(0, |count, pending| {
        //         let key = Key::new(pending.message_id, pending.bridge_id);
        //         let entry: Entry<T> = serde_json::from_value(pending.payload.clone())?;
        //         self.inner.insert(key, entry);
        //         Ok::<usize, serde_json::Error>(count + 1)
        //     })?;

        // tracing::info!(
        //     count,
        //     max_entries = self.config.max_hot_entries,
        //     "restored pending messages from cold storage"
        // );

        let buffer = Arc::clone(&self);
        Ok(tokio::spawn(async move {
            let mut interval = tokio::time::interval(buffer.config.maintenance_interval);
            loop {
                interval.tick().await;
                if let Err(e) = buffer.run().await {
                    tracing::error!(error = ?e, "buffer maintenance failed");
                }
            }
        }))
    }
}

async fn fetch_existing_cursors(
    cursor_builder: &CursorBlocksBuilder,
    tx: &DatabaseTransaction,
) -> Result<Cursors, DbErr> {
    let tuples = cursor_builder
        .inner
        .keys()
        .map(|(bridge_id, chain_id)| (*bridge_id as i32, *chain_id as i64))
        .collect_vec();

    let cursors = indexer_checkpoints::Entity::find()
        .filter(
            Expr::tuple([
                Expr::col(indexer_checkpoints::Column::BridgeId).into(),
                Expr::col(indexer_checkpoints::Column::ChainId).into(),
            ])
            .in_tuples(tuples),
        )
        .all(tx)
        .await?
        .into_iter()
        .map(|model| {
            (
                (model.bridge_id as BridgeId, model.chain_id as ChainId),
                Cursor {
                    backward: model.catchup_max_cursor.max(0) as BlockNumber,
                    forward: model.realtime_cursor.max(0) as BlockNumber,
                },
            )
        })
        .collect();

    Ok(cursors)
}

async fn update_indexer_cursors(
    tx: &DatabaseTransaction,
    cursors: &HashMap<(BridgeId, ChainId), Cursor>,
) -> Result<(), DbErr> {
    let models: Vec<_> = cursors
        .iter()
        .map(
            |((bridge_id, chain_id), cursor)| indexer_checkpoints::ActiveModel {
                bridge_id: ActiveValue::Set(*bridge_id as i32),
                chain_id: ActiveValue::Set(*chain_id as i64),
                catchup_min_cursor: ActiveValue::Set(0),
                catchup_max_cursor: ActiveValue::Set(cursor.backward as i64),
                finality_cursor: ActiveValue::Set(0),
                realtime_cursor: ActiveValue::Set(cursor.forward as i64),
                created_at: ActiveValue::NotSet,
                updated_at: ActiveValue::NotSet,
            },
        )
        .collect();

    indexer_checkpoints::Entity::insert_many(models)
        .on_empty_do_nothing()
        .on_conflict(
            OnConflict::columns([
                indexer_checkpoints::Column::BridgeId,
                indexer_checkpoints::Column::ChainId,
            ])
            .value(
                indexer_checkpoints::Column::CatchupMaxCursor,
                Expr::cust(
                    "LEAST(indexer_checkpoints.catchup_max_cursor, EXCLUDED.catchup_max_cursor)",
                ),
            )
            .value(
                indexer_checkpoints::Column::RealtimeCursor,
                Expr::cust(
                    "GREATEST(indexer_checkpoints.realtime_cursor, EXCLUDED.realtime_cursor)",
                ),
            )
            .value(
                indexer_checkpoints::Column::UpdatedAt,
                Expr::current_timestamp(),
            )
            .to_owned(),
        )
        .exec(tx)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use interchain_indexer_entity::{bridges, sea_orm_active_enums::MessageStatus};

    use super::*;
    use crate::test_utils::init_db;

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DummyMessage {
        // Whether this message is consolidatable (simulates "has send")
        consolidatable: bool,
        // Whether it is final
        is_final: bool,
        // A field we mutate in tests.
        counter: u64,
    }

    impl Consolidate for DummyMessage {
        fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>> {
            if !self.consolidatable {
                return Ok(None);
            }

            Ok(Some(ConsolidatedMessage {
                is_final: self.is_final,
                message: crosschain_messages::ActiveModel {
                    id: ActiveValue::Set(key.message_id),
                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                    status: ActiveValue::Set(MessageStatus::Initiated),
                    ..Default::default()
                },
                transfers: vec![],
            }))
        }
    }

    async fn new_buffer(name: &str) -> Arc<MessageBuffer<DummyMessage>> {
        // These tests need a DB-backed InterchainDatabase because MessageBuffer
        // stores it. We don't *intend* to hit the DB when using alter (hot
        // tier), but constructing the buffer requires a DB handle.
        let db = init_db(name).await;
        let db = InterchainDatabase::new(db.client());
        let config = Config {
            _max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        MessageBuffer::new(db, config)
    }

    #[test]
    fn test_entry_needs_flush_tracks_last_flushed_version() {
        let mut entry = BufferItem::new(DummyMessage {
            consolidatable: true,
            is_final: false,
            counter: 0,
        });

        // Newly created entry: considered flushed at version=0.
        assert!(!entry.is_dirty());

        // After a change, version increments and needs_flush becomes true.
        entry.touch().unwrap();
        assert!(entry.is_dirty());

        // Simulate successful flush.
        entry.last_flushed_version = entry.version;
        assert!(!entry.is_dirty());

        // Another modification should require a flush again.
        entry.touch().unwrap();
        assert!(entry.is_dirty());
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_mutates_existing_hot_entry_and_updates_cursors_version() {
        let buffer = new_buffer("buffer_alter_hot").await;
        let key = Key::new(1, 1);

        buffer
            .inner
            .insert(key, BufferItem::new(DummyMessage::default()));

        buffer
            .alter(key, 100, 123, |m| {
                m.counter += 1;
                Ok(())
            })
            .await
            .unwrap();

        let entry = buffer.inner.get(&key).expect("entry must exist");
        assert_eq!(entry.inner.counter, 1);
        assert_eq!(entry.version, 1, "touch() must bump version");
        let blocks = entry
            .touched_blocks
            .get(&100)
            .expect("touched blocks missing");
        assert_eq!(blocks.as_slice(), &[123]);
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_inserts_default_when_missing() {
        let buffer = new_buffer("buffer_alter_default").await;
        let key = Key::new(2, 1);

        buffer
            .alter(key, 200, 7, |m| {
                // default counter is 0
                m.counter += 10;
                Ok(())
            })
            .await
            .unwrap();

        let entry = buffer.inner.get(&key).expect("entry must exist");
        assert_eq!(entry.inner.counter, 10);
        let blocks = entry
            .touched_blocks
            .get(&200)
            .expect("touched blocks missing");
        assert_eq!(blocks.as_slice(), &[7]);
        assert_eq!(entry.version, 1);
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_concurrent_serializes_per_key_no_deadlock() {
        let buffer = new_buffer("buffer_alter_concurrent").await;
        let key = Key::new(3, 1);

        // Run several concurrent alterations on the same key.
        let tasks = (0..32u64).map(|i| {
            let buffer = buffer.clone();
            async move {
                buffer
                    .alter(key, 300, i, |m| {
                        m.counter += 1;
                        Ok(())
                    })
                    .await
            }
        });

        let results = futures::future::join_all(tasks).await;
        for r in results {
            r.unwrap();
        }

        let entry = buffer.inner.get(&key).expect("entry must exist");
        assert_eq!(entry.inner.counter, 32);
        assert_eq!(entry.version, 32);
        // Concurrent updates should serialize per key; cursor range should cover all blocks.
        let blocks = entry
            .touched_blocks
            .get(&300)
            .expect("touched blocks missing");
        let mut sorted = blocks.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..=31).collect::<Vec<_>>());
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_loads_from_cold_tier_then_mutates() {
        let test_db = init_db("buffer_alter_cold_then_mutate").await;
        let db = InterchainDatabase::new(test_db.client());
        let config = Config {
            _max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        let key = Key::new(10, 1);
        let cold_entry = BufferItem::new(DummyMessage {
            consolidatable: false,
            is_final: false,
            counter: 5,
        });

        // Ensure FK prerequisites exist.
        // pending_messages.bridge_id references bridges table.
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(key.bridge_id as i32),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();

        // Seed cold tier.
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            payload: ActiveValue::Set(serde_json::to_value(&cold_entry).unwrap()),
            created_at: ActiveValue::Set(Some(Utc::now().naive_utc())),
        })
        .await
        .unwrap();

        // Sanity: not in hot tier before access.
        assert!(buffer.inner.get(&key).is_none());

        // Mutate via alter: should load from cold tier (pending_messages), promote to hot tier,
        // then apply mutation and cursor/version updates.
        buffer
            .alter(key, 777, 42, |m: &mut DummyMessage| {
                m.counter += 1;
                Ok(())
            })
            .await
            .unwrap();

        let entry = buffer
            .inner
            .get(&key)
            .expect("entry must be promoted to hot tier");
        assert_eq!(entry.inner.counter, 6);
        assert_eq!(entry.version, 1);
        let blocks = entry
            .touched_blocks
            .get(&777)
            .expect("touched blocks missing");
        assert_eq!(blocks.as_slice(), &[42]);
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_restore_resets_created_at_for_hot_ttl() {
        let test_db = init_db("buffer_restore_created_at").await;
        let db = InterchainDatabase::new(test_db.client());
        let config = Config {
            _max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        let key = Key::new(11, 1);

        // Ensure FK prerequisites exist.
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(key.bridge_id as i32),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();

        let payload_created_at = DateTime::from_timestamp(1_000, 0).unwrap().naive_utc();
        let mut cold_entry = BufferItem::new(DummyMessage {
            consolidatable: false,
            is_final: false,
            counter: 0,
        });
        cold_entry.hot_since = payload_created_at;

        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id as i32),
            payload: ActiveValue::Set(serde_json::to_value(&cold_entry).unwrap()),
            created_at: ActiveValue::NotSet,
        })
        .await
        .unwrap();

        // Trigger restore by mutating (which calls get_mut_or_default).
        buffer
            .alter(key, 1, 1, |_m: &mut DummyMessage| Ok(()))
            .await
            .unwrap();

        let entry = buffer.inner.get(&key).expect("entry must be restored");

        // created_at must not come from payload nor from the pending_messages row timestamp.
        assert_ne!(entry.hot_since, payload_created_at);

        // created_at should be set to "now" on restore (allow some slack).
        let now = Utc::now().naive_utc();
        let drift = now
            .signed_duration_since(entry.hot_since)
            .num_seconds()
            .abs();

        assert!(
            drift <= 2,
            "expected restored created_at to be set near now; drift={}s (entry={}, now={})",
            drift,
            entry.hot_since,
            now
        );
    }
}
