// SPDX-License-Identifier: LicenseRef-Blockscout

pub mod interval;
pub mod policy;
pub mod settings;

pub use interval::*;
pub use policy::*;
pub use settings::*;

use std::{collections::HashSet, sync::Arc};

use crate::InterchainDatabase;

/// Facade over the `indexer_failures` table plus an in-memory "which pairs
/// currently have open holes" cache.
///
/// **Single-writer assumption.** This cache is only safe because one service
/// replica indexes a given `(bridge_id, chain_id)` pair — the same assumption
/// checkpointing already depends on implicitly. The cache may be stale-`true`
/// (one redundant `SELECT`, harmless) but must never be stale-`false`: if a
/// second writer could record a failure this process does not know about,
/// `resolve` would skip the database entirely and silently leave that failure
/// unresolved forever.
pub struct FailureLedger {
    db: Arc<InterchainDatabase>,
    /// Pairs known to have open holes. Absence => `resolve` performs ZERO DB
    /// statements.
    pairs_with_holes: Arc<parking_lot::RwLock<HashSet<(i32, i64)>>>,
}

impl FailureLedger {
    pub fn new(db: Arc<InterchainDatabase>) -> Self {
        Self {
            db,
            pairs_with_holes: Arc::new(parking_lot::RwLock::new(HashSet::new())),
        }
    }

    /// Warm the cache from the database. Called once by `RangeDriver::run`
    /// before the loop starts.
    pub async fn initialize(&self, pairs: &[(i32, i64)]) -> anyhow::Result<()> {
        let open = self.db.open_indexer_failures(pairs).await?;

        let cache: HashSet<(i32, i64)> = open
            .into_iter()
            .map(|(bridge_id, chain_id, _interval)| (bridge_id, chain_id))
            .collect();

        // parking_lot::RwLock: consistent with the surrounding indexer state
        // (avalanche/mod.rs, amb/indexer.rs, database.rs), not held across an
        // `.await` here or anywhere else in this type.
        *self.pairs_with_holes.write() = cache;

        Ok(())
    }

    /// UNION. Inserts the pair into the cache on success.
    pub async fn record(
        &self,
        bridge_id: i32,
        chain_id: i64,
        ranges: &[(BlockRange, String)],
    ) -> anyhow::Result<()> {
        self.db
            .record_indexer_failures(bridge_id, chain_id, ranges)
            .await?;

        crate::indexer::metrics::FAILURE_LEDGER_WRITES_TOTAL
            .with_label_values(&[&bridge_id.to_string(), "record"])
            .inc();

        self.pairs_with_holes.write().insert((bridge_id, chain_id));

        Ok(())
    }

    /// DIFFERENCE. No-op (no DB statement) when the pair is absent from the
    /// cache. Removes the pair from the cache only when the transaction
    /// proved zero rows remain.
    pub async fn resolve(
        &self,
        bridge_id: i32,
        chain_id: i64,
        ranges: &[BlockRange],
    ) -> anyhow::Result<()> {
        // Read membership into a bool and drop the guard before awaiting —
        // never hold a lock guard across an `.await` point.
        let has_holes = self
            .pairs_with_holes
            .read()
            .contains(&(bridge_id, chain_id));
        if !has_holes {
            return Ok(());
        }

        let is_empty = self
            .db
            .resolve_indexer_failures(bridge_id, chain_id, ranges)
            .await?;

        crate::indexer::metrics::FAILURE_LEDGER_WRITES_TOTAL
            .with_label_values(&[&bridge_id.to_string(), "resolve"])
            .inc();

        if is_empty {
            self.pairs_with_holes.write().remove(&(bridge_id, chain_id));
        }

        Ok(())
    }

    /// Pure read, delegates to `open_indexer_failures`.
    pub async fn open(
        &self,
        pairs: &[(i32, i64)],
    ) -> anyhow::Result<Vec<(i32, i64, FailedInterval)>> {
        self.db.open_indexer_failures(pairs).await
    }

    /// Pure read, delegates to `indexer_failure_totals`. Exists on the
    /// facade (rather than requiring callers to hold a separate
    /// `Arc<InterchainDatabase>`) so `RangeDriver`'s gauge refresh — the only
    /// caller today — can reach the aggregate through the one handle it
    /// already holds.
    pub async fn failure_totals(
        &self,
        bridge_id: Option<i32>,
        chain_id: Option<i64>,
    ) -> anyhow::Result<Vec<(i32, i64, u64, Option<chrono::NaiveDateTime>)>> {
        self.db.indexer_failure_totals(bridge_id, chain_id).await
    }
}
