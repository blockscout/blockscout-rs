use std::sync::Arc;

use alloy::primitives::{BlockNumber, ChainId};
use anyhow::Result;
use dashmap::{
    DashMap,
    mapref::{entry::Entry as DashEntry, one::RefMut},
};
use tokio::{sync::RwLock, task::JoinHandle};

use super::{
    buffer_item::BufferItem,
    metrics,
    types::{Consolidate, Key},
};
use crate::{InterchainDatabase, settings::MessageBufferSettings};

/// Tiered message buffer with hot (memory) and cold (Postgres) storage.
///
/// This struct is safe to share across tasks; per-message updates are
/// synchronized by the underlying `DashMap`.
pub struct MessageBuffer<T: Consolidate + Default> {
    /// Hot tier: in-memory entries
    pub(super) inner: DashMap<Key, BufferItem<T>>,
    /// Configuration
    pub(super) config: MessageBufferSettings,
    /// Database connection for cold tier operations
    pub(super) db: InterchainDatabase,
    /// Lock for maintenance operations (offload/flush)
    pub(super) maintenance_lock: RwLock<()>,
}

impl<T: Consolidate + Default> MessageBuffer<T> {
    /// Create a new tiered message buffer.
    ///
    /// Note: no data is eagerly loaded from cold storage. Entries are restored
    /// on-demand via `get_mut_or_default` / `alter`.
    pub fn new(db: InterchainDatabase, config: MessageBufferSettings) -> Arc<Self> {
        Arc::new(Self {
            inner: DashMap::new(),
            config,
            db,
            maintenance_lock: RwLock::new(()),
        })
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

    /// Start the background maintenance loop.
    pub async fn start(self: Arc<Self>) -> Result<JoinHandle<()>> {
        let buffer = Arc::clone(&self);
        Ok(tokio::spawn(async move {
            let mut interval = tokio::time::interval(buffer.config.maintenance_interval);
            loop {
                interval.tick().await;
                if let Err(e) = buffer.run().await {
                    metrics::BUFFER_MAINTENANCE_ERRORS_TOTAL.inc();
                    tracing::error!(error = ?e, "buffer maintenance failed");
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use chrono::DateTime;
    use interchain_indexer_entity::{
        bridges, pending_messages, sea_orm_active_enums::MessageStatus,
    };
    use sea_orm::ActiveValue;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::{message_buffer::ConsolidatedMessage, test_utils::init_db};

    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct DummyMessage {
        consolidatable: bool,
        is_final: bool,
        counter: u64,
    }

    impl Consolidate for DummyMessage {
        fn consolidate(&self, key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
            if !self.consolidatable {
                return Ok(None);
            }

            Ok(Some(ConsolidatedMessage {
                is_final: self.is_final,
                message: interchain_indexer_entity::crosschain_messages::ActiveModel {
                    id: ActiveValue::Set(key.message_id),
                    bridge_id: ActiveValue::Set(key.bridge_id as i32),
                    status: ActiveValue::Set(MessageStatus::Initiated),
                    ..Default::default()
                },
                transfers: vec![],
            }))
        }
    }

    fn test_buffer_settings() -> MessageBufferSettings {
        MessageBufferSettings {
            hot_ttl: Duration::from_secs(60),
            maintenance_interval: Duration::from_secs(60),
        }
    }

    async fn new_buffer(name: &str) -> Arc<MessageBuffer<DummyMessage>> {
        // These tests need a DB-backed InterchainDatabase because MessageBuffer
        // stores it. We don't *intend* to hit the DB when using alter (hot
        // tier), but constructing the buffer requires a DB handle.
        let db = init_db(name).await;
        let db = crate::InterchainDatabase::new(db.client());
        MessageBuffer::new(db, test_buffer_settings())
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
        assert!(blocks.contains(&123));
        assert_eq!(blocks.len(), 1);
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
        assert!(blocks.contains(&7));
        assert_eq!(blocks.len(), 1);
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
        let mut sorted: Vec<_> = blocks.iter().copied().collect();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..=31).collect::<Vec<_>>());
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_alter_loads_from_cold_tier_then_mutates() {
        let test_db = init_db("buffer_alter_cold_then_mutate").await;
        let db = crate::InterchainDatabase::new(test_db.client());
        let buffer = MessageBuffer::new(db.clone(), test_buffer_settings());

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
            created_at: ActiveValue::Set(Some(chrono::Utc::now().naive_utc())),
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
        assert!(blocks.contains(&42));
        assert_eq!(blocks.len(), 1);
    }

    #[tokio::test]
    #[ignore = "needs database to construct InterchainDatabase"]
    async fn test_restore_resets_created_at_for_hot_ttl() {
        let test_db = init_db("buffer_restore_created_at").await;
        let db = crate::InterchainDatabase::new(test_db.client());
        let buffer = MessageBuffer::new(db.clone(), test_buffer_settings());

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
        let now = chrono::Utc::now().naive_utc();
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
