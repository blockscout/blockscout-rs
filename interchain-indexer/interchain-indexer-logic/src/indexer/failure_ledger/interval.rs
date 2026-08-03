// SPDX-License-Identifier: LicenseRef-Blockscout

use chrono::NaiveDateTime;

/// A block range, inclusive on both ends, matching `eth_getLogs`
/// `fromBlock`/`toBlock`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockRange {
    pub from: u64,
    pub to: u64,
}

impl BlockRange {
    /// Number of blocks covered by this range, inclusive on both ends.
    pub fn width(&self) -> u64 {
        self.to.saturating_sub(self.from).saturating_add(1)
    }
}

/// A recorded failed interval, mirroring one `indexer_failures` row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FailedInterval {
    pub range: BlockRange,
    pub attempts: u32,
    pub reason: Option<String>,
    /// `indexer_failures.created_at` — when this interval (or its oldest
    /// merged-in ancestor) was first recorded.
    pub first_failed_at: NaiveDateTime,
    /// `indexer_failures.updated_at` — when this row was last written.
    pub last_attempt_at: NaiveDateTime,
}

/// True when `a` and `b` overlap or are adjacent (no healthy block between
/// them). This is the `record` (union) predicate: adjacency must merge so
/// realtime's `from_block = to_block + 1` advance never grows the table
/// unboundedly.
///
/// Uses `saturating_add` on the upper side rather than subtracting 1 from
/// `from` so a range starting at block `0` cannot underflow.
pub fn overlaps_or_adjacent(a: BlockRange, b: BlockRange) -> bool {
    a.from <= b.to.saturating_add(1) && b.from <= a.to.saturating_add(1)
}

/// True when `a` and `b` share at least one block. This is the `resolve`
/// (difference) predicate — adjacency is irrelevant to subtraction.
pub fn overlaps(a: BlockRange, b: BlockRange) -> bool {
    a.from <= b.to && b.from <= a.to
}

/// The smallest range containing both `a` and `b`.
pub fn merge_bounds(a: BlockRange, b: BlockRange) -> BlockRange {
    BlockRange {
        from: a.from.min(b.from),
        to: a.to.max(b.to),
    }
}

/// `row` minus `sub`, as 0, 1 or 2 disjoint pieces:
/// - `sub` covers `row` (exact match or superset) → empty.
/// - `sub` removes a prefix or a suffix of `row` → one piece.
/// - `sub` sits strictly inside `row` → two pieces.
/// - `row` and `sub` do not overlap → `row` unchanged.
pub fn subtract(row: BlockRange, sub: BlockRange) -> Vec<BlockRange> {
    if !overlaps(row, sub) {
        return vec![row];
    }

    let mut pieces = Vec::with_capacity(2);
    if sub.from > row.from {
        pieces.push(BlockRange {
            from: row.from,
            to: sub.from - 1,
        });
    }
    if sub.to < row.to {
        pieces.push(BlockRange {
            from: sub.to + 1,
            to: row.to,
        });
    }
    pieces
}

/// Normalise a caller-supplied set of ranges by sorting on `from` and folding
/// overlapping/adjacent ranges together. Used to collapse a single `record`
/// call's input before it reaches the database, so a caller passing several
/// touching ranges in one call does not create redundant candidate rows.
///
/// A thin wrapper over [`fold_adjacent`] with a unit payload — the single
/// implementation of the fold lives there so a payload-carrying caller (e.g.
/// `database::pre_union_with_reason`, which carries a `reason` string
/// alongside each range) does not need its own hand-written copy.
pub fn pre_union(ranges: &[BlockRange]) -> Vec<BlockRange> {
    fold_adjacent(ranges.iter().map(|range| (*range, ())).collect())
        .into_iter()
        .map(|(range, ())| range)
        .collect()
}

/// Sort `ranges` by `from` and fold overlapping/adjacent ones together via
/// [`overlaps_or_adjacent`] + [`merge_bounds`], carrying an arbitrary payload
/// alongside each range through the merge. When two inputs merge, the later
/// one's payload wins.
///
/// This is the one implementation the fold logic exists in: [`pre_union`]
/// calls it with a `()` payload, and `database::pre_union_with_reason` calls
/// it directly with a `String` reason — neither hand-rolls its own copy of
/// the sort-and-fold loop.
pub fn fold_adjacent<T>(ranges: Vec<(BlockRange, T)>) -> Vec<(BlockRange, T)> {
    let mut sorted = ranges;
    sorted.sort_by_key(|(range, _)| range.from);

    let mut merged: Vec<(BlockRange, T)> = Vec::with_capacity(sorted.len());
    for (range, payload) in sorted {
        match merged.last_mut() {
            Some((last_range, last_payload)) if overlaps_or_adjacent(*last_range, range) => {
                *last_range = merge_bounds(*last_range, range);
                *last_payload = payload;
            }
            _ => merged.push((range, payload)),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn range(from: u64, to: u64) -> BlockRange {
        BlockRange { from, to }
    }

    #[rstest]
    // subsumed: [1100,1200] is fully inside [1000,2000] -> bounds unchanged
    #[case::subsumed(range(1000, 2000), range(1100, 1200), range(1000, 2000))]
    // overlapping: extends the upper bound
    #[case::overlapping(range(1000, 2000), range(1900, 2500), range(1000, 2500))]
    // adjacent: [1000,1999] + [2000,2999] -> merges into one contiguous range
    #[case::adjacent(range(1000, 1999), range(2000, 2999), range(1000, 2999))]
    fn union_merges_expected_cases(
        #[case] existing: BlockRange,
        #[case] incoming: BlockRange,
        #[case] expected: BlockRange,
    ) {
        assert!(overlaps_or_adjacent(existing, incoming));
        assert_eq!(merge_bounds(existing, incoming), expected);

        let merged = pre_union(&[existing, incoming]);
        assert_eq!(merged, vec![expected]);
    }

    #[test]
    fn union_does_not_merge_across_a_real_gap() {
        let a = range(1000, 2000);
        let b = range(5000, 6000);

        assert!(!overlaps_or_adjacent(a, b));

        let merged = pre_union(&[a, b]);
        assert_eq!(merged, vec![a, b]);
    }

    #[rstest]
    // prefix removed: sub covers the start of row
    #[case::prefix(range(1000, 2000), range(1000, 1099), vec![range(1100, 2000)])]
    // suffix removed: sub covers the end of row
    #[case::suffix(range(1000, 2000), range(1900, 2000), vec![range(1000, 1899)])]
    // strict interior split: sub sits inside row, leaving two pieces
    #[case::interior_split(range(1000, 2000), range(1100, 1200), vec![range(1000, 1099), range(1201, 2000)])]
    // exact match: sub == row -> fully removed
    #[case::exact_match(range(1000, 2000), range(1000, 2000), vec![])]
    // superset: sub fully contains row -> fully removed
    #[case::superset(range(1000, 2000), range(500, 2500), vec![])]
    // disjoint: no overlap -> row unchanged
    #[case::disjoint(range(1000, 2000), range(5000, 6000), vec![range(1000, 2000)])]
    fn difference_produces_expected_pieces(
        #[case] row: BlockRange,
        #[case] sub: BlockRange,
        #[case] expected: Vec<BlockRange>,
    ) {
        assert_eq!(subtract(row, sub), expected);
    }

    #[test]
    fn overlaps_or_adjacent_does_not_underflow_at_from_zero() {
        let a = range(0, 0);
        let b = range(1, 1);

        // [0,0] and [1,1] are adjacent; must not panic computing `from - 1`.
        assert!(overlaps_or_adjacent(a, b));

        let c = range(2, 2);
        assert!(!overlaps_or_adjacent(a, c));
    }

    #[test]
    fn width_is_inclusive_on_both_ends() {
        assert_eq!(range(1000, 1000).width(), 1);
        assert_eq!(range(1000, 1999).width(), 1000);
    }
}
