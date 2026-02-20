use std::collections::{BTreeSet, HashMap};

use alloy::primitives::{BlockNumber, ChainId};
use anyhow::{Context, Result};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

use super::types::Consolidate;

pub(super) type BufferItemVersion = u16;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(bound(serialize = "T: Serialize", deserialize = "T: for<'a> Deserialize<'a>"))]
pub struct BufferItem<T> {
    pub inner: T,
    /// chain_id -> set of block numbers observed for this entry.
    ///
    /// These cursors are used to compute conservative `indexer_checkpoints`
    /// updates during maintenance.
    pub(super) touched_blocks: HashMap<ChainId, BTreeSet<BlockNumber>>,
    /// Monotonic version used to detect concurrent modifications.
    pub(super) version: BufferItemVersion,
    /// Last `version` that was successfully flushed into `crosschain_messages`.
    ///
    /// This is used to avoid repeatedly upserting "consolidated but not final"
    /// entries when they haven't changed.
    #[serde(default)]
    pub(super) last_flushed_version: BufferItemVersion,
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
    pub(super) fn new(inner: T) -> Self {
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
    pub(super) fn record_block(&mut self, chain_id: ChainId, block_number: BlockNumber) -> &Self {
        self.touched_blocks
            .entry(chain_id)
            .or_default()
            .insert(block_number);
        self
    }

    /// Increment version to indicate modification.
    pub(super) fn touch(&mut self) -> Result<&Self> {
        self.version = self
            .version
            .checked_add(1)
            .context("version overflow in message buffer entry")?;
        Ok(self)
    }

    pub(super) fn flushed_at(mut self, version: BufferItemVersion) -> Self {
        self.last_flushed_version = version;
        self
    }

    /// Returns true if this entry has changed since the last successful flush.
    pub(super) fn is_dirty(&self) -> bool {
        self.version > self.last_flushed_version
    }
}
