//! Tiered message buffer for cross-chain message indexing.
//!
//! This module provides a two-tier storage system:
//! - **Hot tier**: In-memory storage for recent messages (fast access)
//! - **Cold tier**: Postgres `pending_messages` table for durability
//!
//! Messages are promoted to `crosschain_messages` when they become "ready".
//!
//! Future improvements:
//! - There might be situations when message is flushed to final storage but
//!   isn't fully indexed. In that case we don't delete it from pending_messages
//!   to allow recovery.

use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use dashmap::{
    DashMap,
    mapref::{entry::Entry as DashEntry, one::RefMut},
};
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, indexer_checkpoints, pending_messages,
};
use sea_orm::{
    ActiveValue, DbErr, EntityTrait, Iterable, QueryFilter, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};

use crate::InterchainDatabase;

/// Key for identifying a cross-chain message
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug, Serialize, Deserialize, Default)]
pub struct Key {
    pub message_id: i64,
    pub bridge_id: i32,
}

impl Key {
    pub fn new(message_id: i64, bridge_id: i32) -> Self {
        Self {
            message_id,
            bridge_id,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ConsolidatedMessage {
    pub is_final: bool,
    pub message: crosschain_messages::ActiveModel,
    pub transfers: Vec<crosschain_transfers::ActiveModel>,
}

pub trait Consolidate:
    Clone + Send + Sync + 'static + Serialize + for<'de> Deserialize<'de>
{
    fn consolidate(&self, key: &Key) -> Result<Option<ConsolidatedMessage>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: for<'a> Deserialize<'a>"))]
pub struct Entry<T> {
    pub inner: T,
    /// chain_id -> (min_block, max_block) seen for this entry
    cursors: HashMap<i64, (i64, i64)>,
    version: u64,
    /// Last `version` that was successfully flushed into `crosschain_messages`.
    ///
    /// This is used to avoid repeatedly upserting "consolidated but not final"
    /// entries when they haven't changed.
    #[serde(default)]
    last_flushed_version: u64,
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

impl<T: Consolidate> Entry<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            cursors: HashMap::new(),
            version: 0,
            last_flushed_version: 0,
            hot_since: now_naive_utc(),
        }
    }

    /// Record that data from a specific block was added to this message.
    /// Updates both min and max to properly track bidirectional cursor movement.
    fn record_block(&mut self, chain_id: i64, block_number: i64) -> &Self {
        self.cursors
            .entry(chain_id)
            .and_modify(|(min, max)| {
                *min = (*min).min(block_number);
                *max = (*max).max(block_number);
            })
            .or_insert((block_number, block_number));
        self
    }

    /// Increment version to indicate modification.
    fn touch(&mut self) -> &Self {
        self.version = self.version.wrapping_add(1);
        self
    }

    fn flushed_at(mut self, version: u64) -> Self {
        self.last_flushed_version = version;
        self
    }

    /// Returns true if this entry has changed since the last successful flush.
    fn is_dirty(&self) -> bool {
        self.version > self.last_flushed_version
    }
}

fn with_cursors<T>(
    key: Key,
    entry: &Entry<T>,
    cursors: HashMap<(i32, i64), (i64, i64)>,
) -> HashMap<(i32, i64), (i64, i64)> {
    entry
        .cursors
        .iter()
        .fold(cursors, |mut acc, (&chain_id, &(min, max))| {
            acc.entry((key.bridge_id, chain_id))
                .and_modify(|(existing_min, existing_max)| {
                    *existing_min = (*existing_min).min(min);
                    *existing_max = (*existing_max).max(max);
                })
                .or_insert((min, max));
            acc
        })
}

/// Configuration for the tiered message buffer
#[derive(Clone, Debug)]
pub struct Config {
    /// Maximum entries in hot tier before forced offload
    pub max_hot_entries: usize,
    /// Time before entry moves from hot to cold
    pub hot_ttl: Duration,
    /// How often to run maintenance (offload + flush)
    pub maintenance_interval: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_hot_entries: 100_000,
            hot_ttl: Duration::from_secs(10),
            maintenance_interval: Duration::from_millis(500),
        }
    }
}

/// Tiered message buffer with hot (memory) and cold (Postgres) storage
pub struct MessageBuffer<T: Consolidate> {
    /// Hot tier: in-memory entries
    inner: DashMap<Key, Entry<T>>,
    /// Configuration
    config: Config,
    /// Database connection for cold tier operations
    db: InterchainDatabase,
    /// Lock for maintenance operations (offload/flush)
    maintenance_lock: RwLock<()>,
}

impl<T: Consolidate> MessageBuffer<T> {
    /// Create a new tiered message buffer
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

    async fn restore(&self, key: Key) -> Result<Option<Entry<T>>> {
        self.db
            .get_pending_message(key.message_id, key.bridge_id)
            .await?
            .map(|row| {
                serde_json::from_value(row.payload).context("failed to deserialize MessageEntry")
            })
            .transpose()
    }

    /// Get existing entry.
    ///
    /// Checks:
    /// 1. hot tier
    /// 2. cold tier (pending_messages)
    pub async fn _get_mut(&self, key: Key) -> Result<Option<RefMut<'_, Key, Entry<T>>>> {
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
    pub async fn get_mut_or_default(&self, key: Key) -> Result<RefMut<'_, Key, Entry<T>>>
    where
        T: Default,
    {
        match self.inner.get_mut(&key) {
            Some(entry) => Ok(entry),
            None => {
                let value = self
                    .restore(key)
                    .await?
                    .unwrap_or_else(|| Entry::new(T::default()));

                let entry = match self.inner.entry(key) {
                    DashEntry::Occupied(entry) => entry.into_ref(),
                    DashEntry::Vacant(entry) => entry.insert(value),
                };

                Ok(entry)
            }
        }
    }

    /// If hot tier exceeds capacity, runs maintenance to flush entries.
    async fn _maybe_run(&self) -> Result<()> {
        if self.inner.len() > self.config.max_hot_entries {
            self.run().await?;
        }
        Ok(())
    }

    /// Get-or-create the entry, mutate its inner message, and upsert while
    /// recording cursors.
    ///
    /// This is a convenience helper for indexers that apply small incremental
    /// updates.
    pub async fn alter(
        &self,
        key: Key,
        chain_id: i64,
        block_number: i64,
        mutator: impl FnOnce(&mut T) -> Result<()>,
    ) -> Result<()>
    where
        T: Default,
    {
        let mut entry = self.get_mut_or_default(key).await?;
        mutator(&mut entry.inner)?;
        entry.record_block(chain_id, block_number);
        entry.touch();
        Ok(())
    }

    /// Run maintenance: offload stale entries, flush ready entries, update
    /// cursors
    ///
    /// Cursor update logic:
    /// - We can only safely advance cursors past blocks where ALL messages have
    ///   been flushed
    /// - Entries still in hot tier or cold storage represent "pending" work
    /// - The realtime_cursor should not advance past the lowest max_block of
    ///   any pending entry
    /// - The catchup_max_block should not retreat past the highest min_block of
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
        let mut keys_to_remove_from_hot: Vec<(Key, u64)> = Vec::new();
        let mut keys_to_remove_from_pending = Vec::new();
        let mut hot_cursors = HashMap::<(_, _), (_, _)>::new();
        let mut cold_cursors = HashMap::<(_, _), (_, _)>::new();

        // Track which keys were flushed so we can update `last_flushed_version` after commit.
        // (We only need this for non-final entries that remain in hot tier.)
        let mut keys_to_flush: Vec<(Key, u64)> = Vec::new();

        // Diagnostics: why entries are not being removed
        let mut not_consolidatable_count = 0usize;
        let mut consolidated_not_final_count = 0usize;
        let mut final_count = 0usize;

        for entry in self.inner.iter() {
            let key = *entry.key();
            let entry_version = entry.version;
            let created_at = entry.hot_since;

            if !entry.is_dirty() {
                continue;
            }

            let consolidated = entry.inner.consolidate(&key)?;
            let is_final = consolidated.as_ref().is_some_and(|c| c.is_final);
            let is_stale = now.signed_duration_since(created_at).to_std()? >= self.config.hot_ttl;
            let should_remove = is_final || is_stale;

            match (&consolidated, is_final) {
                (None, _) => not_consolidatable_count += 1,
                (Some(_), true) => final_count += 1,
                (Some(_), false) => consolidated_not_final_count += 1,
            }

            if let Some(message) = consolidated {
                if is_final {
                    consolidated_entries.push(message);
                    keys_to_remove_from_pending.push(key);
                } else if entry.is_dirty() {
                    consolidated_entries.push(message);
                    keys_to_flush.push((key, entry_version));
                }
            }

            if should_remove {
                if !is_final {
                    stale_entries.push((key, entry.clone()));
                }
                keys_to_remove_from_hot.push((key, entry_version));
                cold_cursors = with_cursors(key, entry.value(), cold_cursors);
            } else {
                hot_cursors = with_cursors(key, entry.value(), hot_cursors);
            }
        }

        let cursors: HashMap<(i32, i64), (i64, i64)> = cold_cursors
            .into_iter()
            .map(|(key, (cold_min, cold_max))| {
                hot_cursors
                    .get(&key)
                    .map(|&(hot_min, hot_max)| {
                        // Bound cursor updates by what's still pending in hot tier
                        // NOTE: this may cause some blocks to be processed twice,
                        // so handlers and inserts should be idempotent.
                        let min = cold_min.max(hot_min.saturating_add(1));
                        let max = cold_max.min(hot_max.saturating_sub(1));
                        (key, (min, max))
                    })
                    .unwrap_or((key, (cold_min, cold_max)))
            })
            .filter(|(_, (min, max))| max >= min)
            .collect();

        let ready_count = consolidated_entries.len();
        let stale_count = stale_entries.len();
        let cursor_count = cursors.len();

        self.db
            .db
            .transaction::<_, (), DbErr>(|tx| {
                Box::pin(async move {
                    let batch_size = (u16::MAX - 100) as usize / pending_messages::Column::iter().count();
                    for batch in stale_entries.chunks(batch_size) {
                        let models: Vec<pending_messages::ActiveModel> = batch
                            .iter()
                            .filter_map(|(key, entry)| {
                                let payload = serde_json::to_value(entry).ok()?;
                                Some(pending_messages::ActiveModel {
                                    message_id: ActiveValue::Set(key.message_id),
                                    bridge_id: ActiveValue::Set(key.bridge_id),
                                    payload: ActiveValue::Set(payload),
                                    created_at: ActiveValue::Set(Some(entry.hot_since)),
                                })
                            })
                            .collect();

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
                    let batch_size = (u16::MAX - 100) as usize / crosschain_messages::Column::iter().count();
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

                    let batch_size = (u16::MAX - 100) as usize / crosschain_transfers::Column::iter().count();
                    // TODO: refactor to avoid deleting then inserting
                    for batch in transfers.chunks(batch_size) {
                        // Delete existing transfers for these messages
                        crosschain_transfers::Entity::insert_many(batch.to_vec())
                            .on_conflict(
                                OnConflict::columns([
                                    crosschain_transfers::Column::MessageId,
                                    crosschain_transfers::Column::BridgeId,
                                    crosschain_transfers::Column::Index,
                                ])
                                .update_columns(
                                    crosschain_transfers::Column::iter()
                                        .filter(|c| !matches!(
                                            c,
                                            crosschain_transfers::Column::Id
                                                | crosschain_transfers::Column::MessageId
                                                | crosschain_transfers::Column::BridgeId
                                                | crosschain_transfers::Column::Index
                                                | crosschain_transfers::Column::CreatedAt
                                        ))
                                )
                                .to_owned(),
                            )
                            .exec(tx)
                            .await?;
                    }

                    let batch_size = (u16::MAX - 100) as usize / pending_messages::Column::iter().count();
                    for batch in keys_to_remove_from_pending.chunks(batch_size) {
                        let batch = batch.iter()
                            .map(|k| (k.message_id, k.bridge_id));

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

                    let models: Vec<_> = cursors
                        .iter()
                        .map(|((bridge_id, chain_id), (min, max))| {
                            indexer_checkpoints::ActiveModel {
                                bridge_id: ActiveValue::Set(*bridge_id as i64),
                                chain_id: ActiveValue::Set(*chain_id),
                                catchup_min_block: ActiveValue::Set(0),
                                catchup_max_block: ActiveValue::Set(*min),
                                finality_cursor: ActiveValue::Set(0),
                                realtime_cursor: ActiveValue::Set(*max),
                                created_at: ActiveValue::NotSet,
                                updated_at: ActiveValue::NotSet,
                            }
                        })
                        .collect();

                        indexer_checkpoints::Entity::insert_many(models)
                            .on_empty_do_nothing()
                            .on_conflict(
                                OnConflict::columns([
                                    indexer_checkpoints::Column::BridgeId,
                                    indexer_checkpoints::Column::ChainId,
                                ])
                                .value(
                                    indexer_checkpoints::Column::CatchupMaxBlock,
                                    Expr::cust(
                                        "LEAST(indexer_checkpoints.catchup_max_block, EXCLUDED.catchup_max_block)",
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
        let mut removed_count = 0;
        let mut skipped_count = 0;
        for (key, expected_version) in keys_to_remove_from_hot {
            if self
                .inner
                .remove_if(&key, |_, entry| entry.version == expected_version)
                .is_some()
            {
                removed_count += 1;
            } else {
                skipped_count += 1;
            }
        }

        tracing::info!(
            ready = ready_count,
            stale = stale_count,
            cursors = cursor_count,
            removed = removed_count,
            skipped = skipped_count,
            hot_len = self.inner.len(),
            not_consolidatable = not_consolidatable_count,
            consolidated_not_final = consolidated_not_final_count,
            r#final = final_count,
            "maintenance completed"
        );

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
                    bridge_id: ActiveValue::Set(key.bridge_id),
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
            max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        MessageBuffer::new(db, config)
    }

    #[test]
    fn test_entry_needs_flush_tracks_last_flushed_version() {
        let mut entry = Entry::new(DummyMessage {
            consolidatable: true,
            is_final: false,
            counter: 0,
        });

        // Newly created entry: considered flushed at version=0.
        assert!(!entry.is_dirty());

        // After a change, version increments and needs_flush becomes true.
        entry.touch();
        assert!(entry.is_dirty());

        // Simulate successful flush.
        entry.last_flushed_version = entry.version;
        assert!(!entry.is_dirty());

        // Another modification should require a flush again.
        entry.touch();
        assert!(entry.is_dirty());
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_mutates_existing_hot_entry_and_updates_cursors_version() {
        let buffer = new_buffer("buffer_alter_hot").await;
        let key = Key::new(1, 1);

        buffer
            .inner
            .insert(key, Entry::new(DummyMessage::default()));

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
        assert_eq!(entry.cursors.get(&100), Some(&(123, 123)));
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
        assert_eq!(entry.cursors.get(&200), Some(&(7, 7)));
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
                    .alter(key, 300, i as i64, |m| {
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
        assert_eq!(entry.cursors.get(&300), Some(&(0, 31)));
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_loads_from_cold_tier_then_mutates() {
        let test_db = init_db("buffer_alter_cold_then_mutate").await;
        let db = InterchainDatabase::new(test_db.client());
        let config = Config {
            max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        let key = Key::new(10, 1);
        let cold_entry = Entry::new(DummyMessage {
            consolidatable: false,
            is_final: false,
            counter: 5,
        });

        // Ensure FK prerequisites exist.
        // pending_messages.bridge_id references bridges table.
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(key.bridge_id),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();

        // Seed cold tier.
        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id),
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
        assert_eq!(entry.cursors.get(&777), Some(&(42, 42)));
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_restore_resets_created_at_for_hot_ttl() {
        let test_db = init_db("buffer_restore_created_at").await;
        let db = InterchainDatabase::new(test_db.client());
        let config = Config {
            max_hot_entries: usize::MAX,
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        };
        let buffer = MessageBuffer::new(db.clone(), config);

        let key = Key::new(11, 1);

        // Ensure FK prerequisites exist.
        db.upsert_bridges(vec![bridges::ActiveModel {
            id: ActiveValue::Set(key.bridge_id),
            name: ActiveValue::Set("test_bridge".to_string()),
            enabled: ActiveValue::Set(true),
            ..Default::default()
        }])
        .await
        .unwrap();

        let payload_created_at = DateTime::from_timestamp(1_000, 0).unwrap().naive_utc();
        let mut cold_entry = Entry::new(DummyMessage {
            consolidatable: false,
            is_final: false,
            counter: 0,
        });
        cold_entry.hot_since = payload_created_at;

        db.upsert_pending_message(pending_messages::ActiveModel {
            message_id: ActiveValue::Set(key.message_id),
            bridge_id: ActiveValue::Set(key.bridge_id),
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
