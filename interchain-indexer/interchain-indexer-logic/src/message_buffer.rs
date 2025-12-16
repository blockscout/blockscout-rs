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
use dashmap::DashMap;
use interchain_indexer_entity::{
    crosschain_messages, crosschain_transfers, indexer_checkpoints, pending_messages,
    sea_orm_active_enums::MessageStatus,
};
use sea_orm::{
    ActiveValue, DbErr, EntityTrait, Iterable, QueryFilter, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};

use crate::InterchainDatabase;

/// Serializable wrapper for MessageStatus (avoids requiring serde on entity crate)
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    #[default]
    Initiated,
    Completed,
    Failed,
}

impl From<MessageStatus> for Status {
    fn from(status: MessageStatus) -> Self {
        match status {
            MessageStatus::Initiated => Status::Initiated,
            MessageStatus::Completed => Status::Completed,
            MessageStatus::Failed => Status::Failed,
        }
    }
}

impl From<Status> for MessageStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Initiated => MessageStatus::Initiated,
            Status::Completed => MessageStatus::Completed,
            Status::Failed => MessageStatus::Failed,
        }
    }
}

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
    pub created_at: NaiveDateTime,
}

impl<T: Consolidate> Entry<T> {
    fn new(inner: T) -> Self {
        Self {
            inner,
            cursors: HashMap::new(),
            version: 0,
            last_flushed_version: 0,
            created_at: Utc::now().naive_utc(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DummyMessage {
        // Whether this message is consolidatable (simulates "has send")
        consolidatable: bool,
        // Whether it is final
        is_final: bool,
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

    #[test]
    fn test_entry_needs_flush_tracks_last_flushed_version() {
        let mut entry = Entry::new(DummyMessage {
            consolidatable: true,
            is_final: false,
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
            hot_ttl: Duration::from_secs(10), // 5 minutes
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

    /// Get existing entry.
    ///
    /// Checks:
    /// 1. hot tier
    /// 2. cold tier (pending_messages)
    pub async fn get(&self, key: Key) -> Result<Option<Entry<T>>> {
        if let Some(entry) = self.inner.get(&key) {
            Ok(entry.clone().into())
        } else {
            self.db
                .get_pending_message(key.message_id, key.bridge_id)
                .await?
                .map(|entry| {
                    serde_json::from_value(entry.payload)
                        .context("failed to deserialize MessageEntry")
                })
                .transpose()
        }
    }

    /// Get existing entry or default.
    pub async fn get_or_default(&self, key: Key) -> Result<Entry<T>>
    where
        T: Default,
    {
        self.get(key)
            .await
            .map(|opt| opt.unwrap_or_else(|| Entry::new(T::default())))
    }

    /// Upsert entry in hot tier.
    /// If hot tier exceeds capacity, runs maintenance to flush entries.
    pub async fn upsert(&self, key: Key, entry: Entry<T>) -> Result<()> {
        self.inner.insert(key, entry);

        // Run maintenance if hot tier is getting full
        if self.inner.len() > self.config.max_hot_entries {
            self.run().await?;
        }

        Ok(())
    }

    /// Convenience method: upsert the inner value and record block cursor.
    /// Creates a new Entry wrapping the inner value, records the block, and upserts.
    pub async fn upsert_with_cursors(
        &self,
        key: Key,
        mut entry: Entry<T>,
        chain_id: i64,
        block_number: i64,
    ) -> Result<()> {
        entry.record_block(chain_id, block_number);
        entry.touch();
        self.upsert(key, entry).await
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
            let created_at = entry.created_at;

            if !entry.is_dirty() {
                continue;
            }

            let consolidated = entry.inner.consolidate(&key)?;
            let is_final = consolidated.as_ref().map_or(false, |c| c.is_final);
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
                                    created_at: ActiveValue::Set(Some(entry.created_at)),
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

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::test_utils::init_db;
//     use interchain_indexer_entity::{
//         bridges, chains, indexer_checkpoints, sea_orm_active_enums::BridgeType,
//     };
//     use sea_orm::ActiveValue::Set;

//     /// Helper to set up test database with required foreign key data
//     async fn setup_test_db(name: &str) -> (InterchainDatabase, i32, i64) {
//         let db = init_db(name).await;
//         let interchain_db = InterchainDatabase::new(db.client());

//         let bridge_id = 1i32;
//         let chain_id = 100i64;

//         // Insert required chain (foreign key for indexer_checkpoints)
//         interchain_db
//             .upsert_chains(vec![chains::ActiveModel {
//                 id: Set(chain_id),
//                 name: Set("Test Chain".to_string()),
//                 native_id: Set(None),
//                 icon: Set(None),
//                 ..Default::default()
//             }])
//             .await
//             .unwrap();

//         // Insert required bridge (foreign key for indexer_checkpoints)
//         interchain_db
//             .upsert_bridges(vec![bridges::ActiveModel {
//                 id: Set(bridge_id),
//                 name: Set("Test Bridge".to_string()),
//                 r#type: Set(Some(BridgeType::AvalancheNative)),
//                 enabled: Set(true),
//                 api_url: Set(None),
//                 ui_url: Set(None),
//                 ..Default::default()
//             }])
//             .await
//             .unwrap();

//         // Insert initial checkpoint
//         indexer_checkpoints::Entity::insert(indexer_checkpoints::ActiveModel {
//             bridge_id: Set(bridge_id as i64),
//             chain_id: Set(chain_id),
//             catchup_min_block: Set(0),
//             catchup_max_block: Set(1000), // Start high, will go DOWN via LEAST
//             finality_cursor: Set(0),
//             realtime_cursor: Set(0), // Start low, will go UP via GREATEST
//             created_at: ActiveValue::NotSet,
//             updated_at: ActiveValue::NotSet,
//         })
//         .exec(interchain_db.db.as_ref())
//         .await
//         .unwrap();

//         (interchain_db, bridge_id, chain_id)
//     }

//     #[test]
//     fn test_message_entry_serialization() {
//         let key = Key::new(12345, 1);
//         let mut msg = Message::new(key);

//         let tx_hash = FixedBytes::<32>::repeat_byte(0xab);
//         msg.src_chain_id = Some(43114);
//         msg.source_transaction_hash = Some(tx_hash);
//         msg.init_timestamp = Some(Utc::now().naive_utc());
//         msg.sender_address = Some(Address::repeat_byte(0x11));
//         msg.native_id = Some(vec![0x22; 32]);
//         msg.status = Status::Completed;
//         msg.cursor.record_block(43114, 1000);

//         let json = serde_json::to_value(&msg).unwrap();
//         let restored = serde_json::from_value::<Message>(json).unwrap();

//         assert_eq!(restored.key, key);
//         assert_eq!(restored.src_chain_id, Some(43114));
//         assert_eq!(restored.source_transaction_hash, Some(tx_hash));
//         assert!(restored.init_timestamp.is_some());
//         assert_eq!(restored.status, Status::Completed);
//         assert_eq!(restored.cursor.inner.get(&43114), Some(&(1000, 1000)));
//     }

//     #[test]
//     fn test_cursor_tracker_bidirectional() {
//         let mut tracker = Cursor::default();

//         tracker.record_block(1, 100);
//         tracker.record_block(1, 200);
//         tracker.record_block(1, 30);

//         assert_eq!(tracker.inner.get(&1), Some(&(30, 200)));
//         assert_eq!(tracker.min_block(1), Some(30));
//         assert_eq!(tracker.max_block(1), Some(200));
//     }

//     /// E2E Test: Ready message is flushed to crosschain_messages table
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_ready_message_flushed_to_final_storage() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_ready_flush").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0), // Immediate TTL for testing
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Create a ready message (has init_timestamp)
//         let mut msg = Message::new(Key::new(1, bridge_id));
//         msg.src_chain_id = Some(chain_id);
//         msg.init_timestamp = Some(Utc::now().naive_utc());
//         msg.cursor.record_block(chain_id, 10);

//         buffer.upsert(msg).await.unwrap();
//         assert_eq!(buffer.inner.len(), 1);

//         // Run maintenance
//         buffer.run().await.unwrap();

//         // Message should be flushed to final storage
//         assert_eq!(
//             buffer.inner.len(),
//             0,
//             "Hot tier should be empty after flush"
//         );

//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(messages.len(), 1, "Message should be in final storage");
//         assert_eq!(messages[0].0.id, 1);
//     }

//     /// E2E Test: Stale message is offloaded to pending_messages table
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_stale_message_offloaded_to_cold_storage() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_stale_offload").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0), // Immediate TTL
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Create an incomplete message (no init_timestamp - not ready)
//         let mut msg = Message::new(Key::new(1, bridge_id));
//         msg.src_chain_id = Some(chain_id);
//         // No init_timestamp - message is not ready
//         msg.cursor.record_block(chain_id, 10);
//         // Backdate created_at to make it stale
//         msg.created_at = Some(Utc::now().naive_utc() - chrono::Duration::seconds(10));

//         buffer.upsert(msg).await.unwrap();
//         assert_eq!(buffer.inner.len(), 1);

//         // Run maintenance
//         buffer.run().await.unwrap();

//         // Message should be offloaded to cold storage (pending_messages)
//         assert_eq!(
//             buffer.inner.len(),
//             0,
//             "Hot tier should be empty after offload"
//         );

//         let pending = db.get_pending_message(1, bridge_id).await.unwrap();
//         assert!(pending.is_some(), "Message should be in pending_messages");
//     }

//     /// E2E Test: Cursor is NOT updated when pending message blocks the range
//     ///
//     /// Scenario:
//     /// - Message 1: ready, at blocks 1-2
//     /// - Message 2: pending (not ready), at block 2
//     ///
//     /// Expected: realtime_cursor should NOT advance because Message 2 at block 2 blocks it
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_cursor_blocked_by_pending_message() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_cursor_blocked").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(300), // Long TTL - don't offload msg2
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Message 1: ready (will be flushed)
//         let mut msg1 = Message::new(Key::new(1, bridge_id));
//         msg1.src_chain_id = Some(chain_id);
//         msg1.init_timestamp = Some(Utc::now().naive_utc());
//         msg1.cursor.record_block(chain_id, 1);
//         msg1.cursor.record_block(chain_id, 2);

//         // Message 2: NOT ready (stays in hot tier)
//         let mut msg2 = Message::new(Key::new(2, bridge_id));
//         msg2.src_chain_id = Some(chain_id);
//         // No init_timestamp
//         msg2.cursor.record_block(chain_id, 2);

//         buffer.upsert(msg1).await.unwrap();
//         buffer.upsert(msg2).await.unwrap();
//         assert_eq!(buffer.inner.len(), 2);

//         // Run maintenance - should flush msg1 but keep msg2
//         buffer.run().await.unwrap();

//         // msg1 flushed, msg2 still in hot
//         assert_eq!(buffer.inner.len(), 1);
//         assert!(buffer.inner.get(&Key::new(2, bridge_id)).is_some());

//         // Check checkpoint - should NOT have been updated due to bounding logic
//         let checkpoint = db
//             .get_checkpoint(bridge_id as u64, chain_id as u64)
//             .await
//             .unwrap()
//             .unwrap();

//         // realtime_cursor should still be 0 (initial value) because msg2 at block 2 blocks it
//         assert_eq!(
//             checkpoint.realtime_cursor, 0,
//             "realtime_cursor should not advance past pending message"
//         );
//     }

//     /// E2E Test: Cursor advances when no pending messages block the range
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_cursor_advances_when_unblocked() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_cursor_advances").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(300),
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Message 1: ready at blocks 1-5
//         let mut msg1 = Message::new(Key::new(1, bridge_id));
//         msg1.src_chain_id = Some(chain_id);
//         msg1.init_timestamp = Some(Utc::now().naive_utc());
//         msg1.cursor.record_block(chain_id, 1);
//         msg1.cursor.record_block(chain_id, 5);

//         // Message 2: NOT ready, but at blocks 10-15 (doesn't overlap with msg1)
//         let mut msg2 = Message::new(Key::new(2, bridge_id));
//         msg2.src_chain_id = Some(chain_id);
//         msg2.cursor.record_block(chain_id, 10);
//         msg2.cursor.record_block(chain_id, 15);

//         buffer.upsert(msg1).await.unwrap();
//         buffer.upsert(msg2).await.unwrap();

//         buffer.run().await.unwrap();

//         // msg1 should be flushed
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(messages.len(), 1);

//         // Due to the bounding logic:
//         // cold: msg1 has blocks 1-5
//         // hot: msg2 has blocks 10-15
//         // min = max(1, 10+1) = 11, max = min(5, 15-1) = 5
//         // Since max(5) < min(11), cursor update is filtered out
//         let checkpoint = db
//             .get_checkpoint(bridge_id as u64, chain_id as u64)
//             .await
//             .unwrap()
//             .unwrap();
//         assert_eq!(
//             checkpoint.realtime_cursor, 0,
//             "Cursor should not update when ranges don't overlap properly"
//         );
//     }

//     /// E2E Test: Message restored from cold storage on start
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_cold_storage_restored_on_start() {
//         let (db, bridge_id, _chain_id) = setup_test_db("buffer_restore").await;

//         // Pre-populate pending_messages
//         let msg = Message::new(Key::new(42, bridge_id));
//         let payload = serde_json::to_value(&msg).unwrap();
//         db.upsert_pending_message(pending_messages::ActiveModel {
//             message_id: Set(42),
//             bridge_id: Set(bridge_id),
//             payload: Set(payload),
//             created_at: Set(Some(Utc::now().naive_utc())),
//         })
//         .await
//         .unwrap();

//         // Create buffer and start (which loads from cold storage)
//         let config = Config::default();
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Start loads pending messages
//         let _handle = buffer.clone().start().await.unwrap();

//         // Should have the message in hot tier
//         assert_eq!(buffer.inner.len(), 1);
//         assert!(buffer.inner.get(&Key::new(42, bridge_id)).is_some());
//     }

//     /// E2E Test: Maintenance triggered when hot tier exceeds capacity
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_maintenance_triggered_on_capacity() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_capacity").await;

//         let config = Config {
//             max_hot_entries: 2, // Very low capacity
//             hot_ttl: Duration::from_secs(0),
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Insert 3 ready messages (exceeds capacity of 2)
//         for i in 1..=3 {
//             let mut msg = Message::new(Key::new(i, bridge_id));
//             msg.src_chain_id = Some(chain_id);
//             msg.init_timestamp = Some(Utc::now().naive_utc());
//             msg.cursor.record_block(chain_id, i);
//             buffer.upsert(msg).await.unwrap();
//         }

//         // After inserting 3rd message, maintenance should have been triggered
//         // All ready messages should be flushed
//         assert_eq!(
//             buffer.inner.len(),
//             0,
//             "Hot tier should be empty after capacity-triggered maintenance"
//         );

//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(messages.len(), 3, "All messages should be in final storage");
//     }

//     /// E2E Test: Pending message deleted when promoted to final storage
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_pending_deleted_on_promotion() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_pending_delete").await;

//         // Pre-populate pending_messages with incomplete message
//         let mut msg = Message::new(Key::new(1, bridge_id));
//         msg.src_chain_id = Some(chain_id);
//         msg.cursor.record_block(chain_id, 5);
//         let payload = serde_json::to_value(&msg).unwrap();

//         db.upsert_pending_message(pending_messages::ActiveModel {
//             message_id: Set(1),
//             bridge_id: Set(bridge_id),
//             payload: Set(payload),
//             created_at: Set(Some(Utc::now().naive_utc())),
//         })
//         .await
//         .unwrap();

//         // Verify it's in pending
//         assert!(
//             db.get_pending_message(1, bridge_id)
//                 .await
//                 .unwrap()
//                 .is_some()
//         );

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0),
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Now complete the message (add init_timestamp) and upsert
//         msg.init_timestamp = Some(Utc::now().naive_utc());
//         buffer.upsert(msg).await.unwrap();

//         // Run maintenance
//         buffer.run().await.unwrap();

//         // Message should be in final storage
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(messages.len(), 1);

//         // Message should be deleted from pending
//         assert!(
//             db.get_pending_message(1, bridge_id)
//                 .await
//                 .unwrap()
//                 .is_none(),
//             "Message should be deleted from pending_messages after promotion"
//         );
//     }

//     /// Regression test: When SendCrossChainMessage is flushed to crosschain_messages,
//     /// and then a later event arrives (e.g., ReceiveCrossChainMessage), the buffer
//     /// should find the existing message in the final table and update it,
//     /// not create a new orphan entry.
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_update_after_message_flushed_to_final_storage() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_update_after_flush").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0), // Immediate TTL for testing
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         let key = Key::new(1, bridge_id);

//         // Phase 1: Insert a READY message (simulating SendCrossChainMessage)
//         // with init_timestamp set and source data filled
//         let mut msg = Message::new(key);
//         msg.init_timestamp = Some(chrono::Utc::now().naive_utc());
//         msg.src_chain_id = Some(chain_id);
//         msg.source_transaction_hash = Some(FixedBytes::<32>::repeat_byte(0xab));
//         msg.sender_address = Some(Address::repeat_byte(0x11));
//         msg.cursor.record_block(chain_id, 1000);
//         buffer.upsert(msg).await.unwrap();

//         // Phase 2: Run maintenance - this flushes the ready message to crosschain_messages
//         buffer.run().await.unwrap();

//         // Verify message is in final storage
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(
//             messages.len(),
//             1,
//             "Message should be in crosschain_messages"
//         );
//         let msg_row = &messages[0].0;
//         assert_eq!(msg_row.src_chain_id, chain_id);
//         assert!(
//             msg_row.dst_chain_id.is_none(),
//             "Dest chain should not be set yet"
//         );
//         assert_eq!(msg_row.status, MessageStatus::Initiated);

//         // Verify hot tier is empty
//         assert!(
//             buffer.inner.is_empty(),
//             "Hot tier should be empty after flush"
//         );

//         // Phase 3: A later event arrives (e.g., message completion)
//         // The handler calls get_or_create and updates with destination data
//         let mut msg = buffer.get_or_create(key).await.unwrap();
//         // The bug: this creates a NEW entry without init_timestamp
//         // It should have found the message in crosschain_messages and loaded it
//         msg.destination_chain_id = Some(chain_id); // Same chain for simplicity
//         msg.destination_transaction_hash = Some(FixedBytes::<32>::repeat_byte(0xef));
//         msg.recipient_address = Some(Address::repeat_byte(0x22));
//         msg.status = Status::Completed;
//         msg.cursor.record_block(chain_id, 2000);
//         buffer.upsert(msg).await.unwrap();

//         // Phase 4: Run maintenance again
//         buffer.run().await.unwrap();

//         // Verify: The message in crosschain_messages should now have BOTH source AND dest data
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(
//             messages.len(),
//             1,
//             "Should still be 1 message, not 2 (no orphan created)"
//         );

//         let msg = &messages[0].0;
//         // Source data should still be there
//         assert_eq!(
//             msg.src_chain_id, chain_id,
//             "Source chain should still be set"
//         );
//         assert!(
//             msg.src_tx_hash.is_some(),
//             "Source tx hash should still be present"
//         );

//         // Destination data should now be added
//         assert_eq!(
//             msg.dst_chain_id,
//             Some(chain_id),
//             "Dest chain should now be set from update event"
//         );
//         assert!(msg.dst_tx_hash.is_some(), "Dest tx hash should now be set");
//         assert_eq!(
//             msg.status,
//             MessageStatus::Completed,
//             "Status should be updated"
//         );
//     }

//     /// Unit test: is_ictt_complete returns true when no ICTT fragments
//     #[test]
//     fn test_is_ictt_complete_no_fragments() {
//         let msg = Message::new(Key::new(1, 1));
//         assert!(msg.is_ictt_complete(), "No fragments means complete");
//     }

//     /// Unit test: is_ictt_complete returns false with only TokensSent (no destination event)
//     #[test]
//     fn test_is_ictt_complete_tokens_sent_only() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments.push(IcttEventFragment::TokensSent {
//             token_contract: Address::repeat_byte(0x01),
//             sender: Address::repeat_byte(0x02),
//             dst_token_address: Address::repeat_byte(0x03),
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             !msg.is_ictt_complete(),
//             "TokensSent without TokensWithdrawn is incomplete"
//         );
//     }

//     /// Unit test: is_ictt_complete returns true with TokensSent + TokensWithdrawn
//     #[test]
//     fn test_is_ictt_complete_tokens_sent_and_withdrawn() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments.push(IcttEventFragment::TokensSent {
//             token_contract: Address::repeat_byte(0x01),
//             sender: Address::repeat_byte(0x02),
//             dst_token_address: Address::repeat_byte(0x03),
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             msg.is_ictt_complete(),
//             "TokensSent + TokensWithdrawn is complete"
//         );
//     }

//     /// Unit test: is_ictt_complete returns false with only TokensAndCallSent (no call result)
//     #[test]
//     fn test_is_ictt_complete_tokens_and_call_sent_only() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments
//             .push(IcttEventFragment::TokensAndCallSent {
//                 token_contract: Address::repeat_byte(0x01),
//                 sender: Address::repeat_byte(0x02),
//                 dst_token_address: Address::repeat_byte(0x03),
//                 recipient_contract: Address::repeat_byte(0x04),
//                 fallback_recipient: Address::repeat_byte(0x05),
//                 amount: alloy::primitives::U256::from(1000),
//             });
//         assert!(
//             !msg.is_ictt_complete(),
//             "TokensAndCallSent without CallSucceeded/CallFailed is incomplete"
//         );
//     }

//     /// Unit test: is_ictt_complete returns true with TokensAndCallSent + CallSucceeded
//     #[test]
//     fn test_is_ictt_complete_tokens_and_call_sent_with_success() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments
//             .push(IcttEventFragment::TokensAndCallSent {
//                 token_contract: Address::repeat_byte(0x01),
//                 sender: Address::repeat_byte(0x02),
//                 dst_token_address: Address::repeat_byte(0x03),
//                 recipient_contract: Address::repeat_byte(0x04),
//                 fallback_recipient: Address::repeat_byte(0x05),
//                 amount: alloy::primitives::U256::from(1000),
//             });
//         msg.ictt_fragments.push(IcttEventFragment::CallSucceeded {
//             recipient_contract: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             msg.is_ictt_complete(),
//             "TokensAndCallSent + CallSucceeded is complete"
//         );
//     }

//     /// Unit test: is_ictt_complete returns true with TokensAndCallSent + CallFailed
//     #[test]
//     fn test_is_ictt_complete_tokens_and_call_sent_with_failure() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments
//             .push(IcttEventFragment::TokensAndCallSent {
//                 token_contract: Address::repeat_byte(0x01),
//                 sender: Address::repeat_byte(0x02),
//                 dst_token_address: Address::repeat_byte(0x03),
//                 recipient_contract: Address::repeat_byte(0x04),
//                 fallback_recipient: Address::repeat_byte(0x05),
//                 amount: alloy::primitives::U256::from(1000),
//             });
//         msg.ictt_fragments.push(IcttEventFragment::CallFailed {
//             recipient_contract: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             msg.is_ictt_complete(),
//             "TokensAndCallSent + CallFailed is complete"
//         );
//     }

//     /// Unit test: is_ictt_complete returns false with mismatched pairs
//     /// (TokensSent should not be completed by CallSucceeded)
//     #[test]
//     fn test_is_ictt_complete_mismatched_pairs() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments.push(IcttEventFragment::TokensSent {
//             token_contract: Address::repeat_byte(0x01),
//             sender: Address::repeat_byte(0x02),
//             dst_token_address: Address::repeat_byte(0x03),
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         msg.ictt_fragments.push(IcttEventFragment::CallSucceeded {
//             recipient_contract: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             !msg.is_ictt_complete(),
//             "TokensSent + CallSucceeded is NOT a valid completion pair"
//         );
//     }

//     /// Unit test: is_ictt_complete returns false with only destination events (no source)
//     #[test]
//     fn test_is_ictt_complete_destination_only() {
//         let mut msg = Message::new(Key::new(1, 1));
//         msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         assert!(
//             !msg.is_ictt_complete(),
//             "TokensWithdrawn without source event is incomplete"
//         );
//     }

//     /// E2E Test: Incomplete ICTT message is flushed but kept in pending
//     ///
//     /// Scenario:
//     /// - Message has init_timestamp (ready) and TokensSent (source ICTT event)
//     /// - But NO destination event yet (TokensWithdrawn)
//     ///
//     /// Expected:
//     /// - Message is flushed to crosschain_messages (it's ready)
//     /// - Message is NOT deleted from pending_messages (ICTT incomplete)
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_incomplete_ictt_kept_in_pending() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_incomplete_ictt").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0), // Immediate TTL
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Create a ready message with incomplete ICTT (only source event)
//         let mut msg = Message::new(Key::new(1, bridge_id));
//         msg.src_chain_id = Some(chain_id);
//         msg.destination_chain_id = Some(chain_id); // Same chain for simplicity in test
//         msg.init_timestamp = Some(Utc::now().naive_utc()); // Ready!
//         msg.cursor.record_block(chain_id, 10);

//         // Add TokensSent but NOT TokensWithdrawn - ICTT incomplete
//         msg.ictt_fragments.push(IcttEventFragment::TokensSent {
//             token_contract: Address::repeat_byte(0x01),
//             sender: Address::repeat_byte(0x02),
//             dst_token_address: Address::repeat_byte(0x03),
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });

//         buffer.upsert(msg).await.unwrap();

//         // Run maintenance
//         buffer.run().await.unwrap();

//         // Message should be flushed to crosschain_messages (it's ready)
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(
//             messages.len(),
//             1,
//             "Message should be in crosschain_messages"
//         );

//         // But pending should still exist because ICTT is incomplete
//         let pending = db.get_pending_message(1, bridge_id).await.unwrap();
//         assert!(
//             pending.is_some(),
//             "Pending should NOT be deleted when ICTT is incomplete"
//         );
//     }

//     /// E2E Test: Complete ICTT message is flushed AND deleted from pending
//     ///
//     /// Scenario:
//     /// - Message has init_timestamp (ready) and complete ICTT (TokensSent + TokensWithdrawn)
//     ///
//     /// Expected:
//     /// - Message is flushed to crosschain_messages
//     /// - Message IS deleted from pending_messages
//     #[tokio::test]
//     #[ignore = "needs database to run"]
//     async fn test_complete_ictt_deleted_from_pending() {
//         let (db, bridge_id, chain_id) = setup_test_db("buffer_complete_ictt").await;

//         let config = Config {
//             max_hot_entries: 100,
//             hot_ttl: Duration::from_secs(0),
//             maintenance_interval: Duration::from_secs(60),
//         };
//         let buffer = MessageBuffer::new(db.clone(), config);

//         // Create a ready message with complete ICTT
//         let mut msg = Message::new(Key::new(1, bridge_id));
//         msg.src_chain_id = Some(chain_id);
//         msg.destination_chain_id = Some(chain_id); // Same chain for simplicity in test
//         msg.init_timestamp = Some(Utc::now().naive_utc());
//         msg.cursor.record_block(chain_id, 10);

//         // Add TokensSent AND TokensWithdrawn - ICTT complete
//         msg.ictt_fragments.push(IcttEventFragment::TokensSent {
//             token_contract: Address::repeat_byte(0x01),
//             sender: Address::repeat_byte(0x02),
//             dst_token_address: Address::repeat_byte(0x03),
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });
//         msg.ictt_fragments.push(IcttEventFragment::TokensWithdrawn {
//             recipient: Address::repeat_byte(0x04),
//             amount: alloy::primitives::U256::from(1000),
//         });

//         buffer.upsert(msg).await.unwrap();

//         // Run maintenance
//         buffer.run().await.unwrap();

//         // Message should be flushed to crosschain_messages
//         let (messages, _) = db
//             .get_crosschain_messages(None, 100, false, None)
//             .await
//             .unwrap();
//         assert_eq!(messages.len(), 1);

//         // Pending should be deleted because ICTT is complete
//         let pending = db.get_pending_message(1, bridge_id).await.unwrap();
//         assert!(
//             pending.is_none(),
//             "Pending should be deleted when ICTT is complete"
//         );
//     }
// }
