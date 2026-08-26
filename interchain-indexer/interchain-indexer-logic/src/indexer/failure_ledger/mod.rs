// SPDX-License-Identifier: LicenseRef-Blockscout

pub mod interval;
pub mod policy;
pub mod settings;

pub use interval::*;
pub use policy::*;
pub use settings::*;

use std::{collections::HashMap, sync::Arc};

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
    /// Pairs known to have open holes, each mapped to a monotone *record
    /// epoch*. Absence => `resolve` performs ZERO DB statements.
    ///
    /// The epoch exists because `record` and `resolve` for the same pair can
    /// interleave: the retry pass runs as a sibling future of the forward
    /// chain handlers (`RangeDriver::run`), and both call into this ledger
    /// for the same `(bridge_id, chain_id)`. Without it, a `record` landing
    /// during a `resolve`'s database round trip would be erased from the
    /// cache by that `resolve`'s "the set is now empty" removal, leaving the
    /// cache stale-`false` and the freshly recorded hole permanently
    /// unresolvable.
    pairs_with_holes: Arc<parking_lot::RwLock<HashMap<(i32, i64), u64>>>,
}

impl FailureLedger {
    pub fn new(db: Arc<InterchainDatabase>) -> Self {
        Self {
            db,
            pairs_with_holes: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        }
    }

    /// Warm the cache from the database. Called once by `RangeDriver::run`
    /// before the loop starts.
    pub async fn initialize(&self, pairs: &[(i32, i64)]) -> anyhow::Result<()> {
        let open = self.db.open_indexer_failures(pairs).await?;

        let cache: HashMap<(i32, i64), u64> = open
            .into_iter()
            .map(|(bridge_id, chain_id, _interval)| ((bridge_id, chain_id), 0))
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
        let pair = (bridge_id, chain_id);
        // Bumped on BOTH sides of the write, and the trailing bump is the one
        // that carries the safety property.
        //
        // A bump before the write alone is NOT enough. `resolve` snapshots the
        // epoch, then asks the database whether any rows remain; its `COUNT`
        // runs in its own transaction and therefore cannot see this call's
        // uncommitted `INSERT`. If the only bump happened before the write, a
        // `resolve` that snapshots after it reads the already-bumped value,
        // its `COUNT` still returns zero, the equality check passes, and it
        // clears the pair — leaving the cache stale-`false` against a row that
        // commits a moment later. That is precisely the mode this epoch exists
        // to prevent.
        //
        // Bumping again after the write closes it: any `resolve` whose `COUNT`
        // missed this row observes a different epoch by the time it compares,
        // and even a `resolve` that already removed the pair is repaired,
        // because `or_insert` re-creates the entry.
        //
        // The leading bump is kept because it is free and strictly
        // conservative: while the write is in flight the pair is present, so a
        // concurrent `resolve` issues a real database check instead of
        // short-circuiting. Both bumps can only ever leave the cache
        // stale-`true` — one redundant `SELECT` — which is the harmless
        // direction.
        self.bump_epoch(pair);

        self.db
            .record_indexer_failures(bridge_id, chain_id, ranges)
            .await?;

        self.bump_epoch(pair);

        crate::indexer::metrics::FAILURE_LEDGER_WRITES_TOTAL
            .with_label_values(&[&bridge_id.to_string(), "record"])
            .inc();

        Ok(())
    }

    /// Marks that a `record` touched `pair`, re-creating the entry if a
    /// concurrent `resolve` removed it.
    fn bump_epoch(&self, pair: (i32, i64)) {
        *self.pairs_with_holes.write().entry(pair).or_insert(0) += 1;
    }

    /// DIFFERENCE. No-op (no DB statement) when the pair is absent from the
    /// cache. Removes the pair from the cache only when the transaction
    /// proved zero rows remain AND no `record` bumped the pair's epoch while
    /// this call's database round trip was in flight.
    pub async fn resolve(
        &self,
        bridge_id: i32,
        chain_id: i64,
        ranges: &[BlockRange],
    ) -> anyhow::Result<()> {
        let pair = (bridge_id, chain_id);
        // Read the epoch into a local and drop the guard before awaiting —
        // never hold a lock guard across an `.await` point.
        let Some(epoch) = self.pairs_with_holes.read().get(&pair).copied() else {
            return Ok(());
        };

        let is_empty = self
            .db
            .resolve_indexer_failures(bridge_id, chain_id, ranges)
            .await?;

        crate::indexer::metrics::FAILURE_LEDGER_WRITES_TOTAL
            .with_label_values(&[&bridge_id.to_string(), "resolve"])
            .inc();

        if is_empty {
            let mut guard = self.pairs_with_holes.write();
            if may_clear_pair(guard.get(&pair).copied(), epoch) {
                guard.remove(&pair);
            }
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
}

/// Whether a `resolve` that observed an empty set may clear the pair from the
/// cache: only when no `record` for that pair bumped the epoch during the
/// database round trip. A changed epoch means the "set is now empty" answer
/// predates a newly recorded hole, and clearing on it would make the cache
/// stale-`false`.
fn may_clear_pair(current_epoch: Option<u64>, snapshot_epoch: u64) -> bool {
    current_epoch == Some(snapshot_epoch)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{BlockRange, FailureLedger, may_clear_pair};
    use crate::{
        InterchainDatabase,
        test_utils::{init_db, mock_db::fill_mock_interchain_database},
    };

    /// Pins WHERE `record` bumps the epoch, which the `may_clear_pair_*` cases
    /// below cannot: they are equality checks over values handed to them, so
    /// they hold for any placement of the bump.
    ///
    /// A bump that happens only *before* the database write is not enough.
    /// `resolve` snapshots the epoch and then asks the database whether rows
    /// remain; that `COUNT` runs in its own transaction and cannot see an
    /// uncommitted `INSERT`. So a `resolve` that snapshots after the leading
    /// bump reads the already-bumped value, its `COUNT` returns zero, the
    /// equality check passes, and it clears the pair — against a row that
    /// commits immediately afterwards. The cache is then stale-`false` and
    /// every later `resolve` for that pair short-circuits with no SQL, so the
    /// hole is replayed forever and can never clear.
    ///
    /// The test therefore drives `record` to its database await, reads the
    /// epoch at that point, and requires it to have moved again by the time
    /// the call returns.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn record_bumps_the_epoch_after_its_write_not_only_before() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;

        let db = init_db("failure_ledger_record_bumps_epoch_after_write").await;
        fill_mock_interchain_database(&db).await;
        let ledger = FailureLedger::new(Arc::new(InterchainDatabase::new(db.client())));
        let pair = (BRIDGE_ID, CHAIN_ID);

        let ranges = [(BlockRange { from: 0, to: 99 }, "boom".to_string())];
        let mut record = Box::pin(ledger.record(BRIDGE_ID, CHAIN_ID, &ranges));

        // One poll takes it into the database round trip, no further.
        assert!(
            futures::poll!(&mut record).is_pending(),
            "record must reach its database await for this test to mean anything"
        );
        let in_flight = ledger.pairs_with_holes.read().get(&pair).copied();

        record.await.expect("record must succeed");
        let after_write = ledger.pairs_with_holes.read().get(&pair).copied();

        assert!(
            in_flight.is_some(),
            "the pair must be visible while the write is in flight, so a \
             concurrent resolve issues a real database check"
        );
        assert!(
            after_write > in_flight,
            "record must bump the epoch again AFTER its write commits \
             ({in_flight:?} -> {after_write:?}); with only a leading bump a \
             concurrent resolve's snapshot already contains the sole bump and \
             it will clear the pair, leaving the cache stale-false"
        );
    }

    #[test]
    fn may_clear_pair_allows_clearing_when_the_epoch_is_unchanged() {
        assert!(may_clear_pair(Some(3), 3));
    }

    #[test]
    fn may_clear_pair_refuses_when_a_record_bumped_the_epoch() {
        assert!(!may_clear_pair(Some(4), 3));
    }

    #[test]
    fn may_clear_pair_refuses_when_the_pair_is_gone() {
        assert!(!may_clear_pair(None, 3));
    }
}
