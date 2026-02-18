// ! Cursor management for bidirectional blockchain indexing.
//!
//! This module provides primitives for tracking scan boundaries when indexing
//! cross-chain messages across multiple blockchains in both directions:
//! - **Backward scanning** (catchup): Processing historical blocks
//! - **Forward scanning** (realtime): Processing new blocks
//!
//! ## Key Concepts
//!
//! - **Cold blocks**: Blocks that have been scanned and contain relevant events.
//!   These blocks represent work that has been completed and can be safely consolidated.
//! - **Hot blocks**: Blocks with incompatible data that act as barriers to consolidation.
//!   These prevent the cursor from advancing past them, as they contain pending work
//!   or unresolved states.
//! - **Bridging gaps**: The cursor can span ranges between cold blocks, treating the gap
//!   as "scanned but empty". The cursor advances right up to (but not including) hot blocks,
//!   representing the furthest extent of scan coverage.
//!
//! ## Usage
//!
//! The cursor operates in two modes:
//! 1. **Bootstrap mode** (`cursor = None`): Finds the longest continuous range of cold blocks
//!    not interrupted by hot blocks. Used when initializing a new indexer or recovering state.
//! 2. **Incremental mode** (`cursor = Some`): Extends existing cursor boundaries toward
//!    new cold blocks, stopping at hot block barriers. Used during normal operation.
//!
//! ## Example
//!
//! ```ignore
//! // Bootstrap: find initial cursor range
//! let cursor = compute_cursor(None, &cold_blocks, &hot_blocks);
//!
//! // Incremental: extend cursor as more blocks are processed
//! let updated = compute_cursor(cursor, &new_cold_blocks, &new_hot_blocks);
//! ```

use std::collections::{BTreeSet, HashMap};

use alloy::primitives::{BlockNumber, ChainId};

pub(crate) type BridgeId = i16;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Cursor {
    pub backward: BlockNumber,
    pub forward: BlockNumber,
}

/// Per-chain cold/hot block sets used for cursor derivation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BlockSets {
    pub(crate) cold: BTreeSet<BlockNumber>,
    pub(crate) hot: BTreeSet<BlockNumber>,
}

impl BlockSets {
    pub(crate) fn extend_cursor(&self, cursor: Cursor) -> Cursor {
        let backward = extend_cursor_boundary(
            ScanDirection::Backward,
            cursor.backward,
            &self.cold,
            &self.hot,
        );
        let forward = extend_cursor_boundary(
            ScanDirection::Forward,
            cursor.forward,
            &self.cold,
            &self.hot,
        );

        Cursor { backward, forward }
    }

    pub(crate) fn bootstrap_cursor(&self) -> Option<Cursor> {
        // Filter out cold blocks that are also hot - these can never be included
        // in a scannable range as they contain blocking data
        let scannable_blocks: Vec<BlockNumber> = self
            .cold
            .iter()
            .copied()
            .filter(|b| !self.hot.contains(b))
            .collect();

        if scannable_blocks.is_empty() {
            return None;
        }

        // Track the longest continuous range found so far
        let mut longest_range_start = scannable_blocks[0];
        let mut longest_range_end = scannable_blocks[0];
        let mut longest_range_width: u64 = 0;

        // Track the current range being evaluated
        let mut current_range_start = scannable_blocks[0];

        // Walk through scannable blocks in order, splitting ranges when
        // hot blocks appear in gaps between consecutive cold blocks
        for i in 1..scannable_blocks.len() {
            let prev = scannable_blocks[i - 1];
            let block = scannable_blocks[i];

            // Check if hot blocks exist in gap (prev + 1)..block
            // If yes, this gap splits the range - start a new range at current block
            if self.hot.range((prev + 1)..block).next().is_some() {
                current_range_start = block;
            }

            // Calculate current range width and update best if longer
            let span = block.saturating_sub(current_range_start);
            if span > longest_range_width {
                longest_range_width = span;
                longest_range_start = current_range_start;
                longest_range_end = block;
            }
        }

        Some(Cursor {
            backward: longest_range_start,
            forward: longest_range_end,
        })
    }
}

pub type Cursors = HashMap<(BridgeId, ChainId), Cursor>;

/// Accumulator for per-bridge, per-chain cold/hot block tracking.
#[derive(Clone, Debug, Default)]
pub(crate) struct CursorBlocksBuilder {
    pub(crate) inner: HashMap<(BridgeId, ChainId), BlockSets>,
}

impl CursorBlocksBuilder {
    pub(crate) fn merge_cold(
        &mut self,
        bridge_id: BridgeId,
        touched_blocks: &HashMap<ChainId, BTreeSet<BlockNumber>>,
    ) {
        self.merge(bridge_id, touched_blocks, |sets, blocks| {
            sets.cold.extend(blocks.iter().copied())
        });
    }

    pub(crate) fn merge_hot(
        &mut self,
        bridge_id: BridgeId,
        touched_blocks: &HashMap<ChainId, BTreeSet<BlockNumber>>,
    ) {
        self.merge(bridge_id, touched_blocks, |sets, blocks| {
            sets.hot.extend(blocks.iter().copied())
        });
    }

    pub(crate) fn calculate_updates(&self, existing_cursors: &Cursors) -> Cursors {
        self.inner
            .iter()
            .filter_map(|(key, sets)| {
                let existing_cursor = existing_cursors.get(key).copied();
                let cursor = match existing_cursor {
                    Some(cursor) => sets.extend_cursor(cursor),
                    None => sets.bootstrap_cursor()?,
                };
                Some((*key, cursor))
            })
            .collect()
    }

    fn merge(
        &mut self,
        bridge_id: BridgeId,
        touched_blocks: &HashMap<ChainId, BTreeSet<BlockNumber>>,
        mut apply: impl FnMut(&mut BlockSets, &BTreeSet<BlockNumber>),
    ) {
        for (&chain_id, blocks) in touched_blocks {
            let sets = self.inner.entry((bridge_id, chain_id)).or_default();
            apply(sets, blocks);
        }
    }
}

/// Direction for cursor extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanDirection {
    Backward,
    Forward,
}

/// Extends a cursor boundary in the specified direction.
///
/// Walks through cold blocks from the current position, bridging gaps
/// between cold blocks until encountering:
/// - A cold block that is also hot (direct barrier)
/// - A hot block in the gap between cold blocks (gap barrier)
///
/// The cursor represents scan coverage, so when a hot block is encountered,
/// the cursor advances to the block immediately before the hot barrier.
///
/// # Arguments
///
/// * `direction` - Direction to extend (Backward or Forward)
/// * `current_boundary` - Starting boundary position
/// * `cold` - Set of cold blocks to walk through
/// * `hot` - Set of hot blocks acting as barriers
///
/// # Returns
///
/// New boundary position (may be unchanged if immediate barrier exists)
fn extend_cursor_boundary(
    direction: ScanDirection,
    current_boundary: BlockNumber,
    cold: &BTreeSet<BlockNumber>,
    hot: &BTreeSet<BlockNumber>,
) -> BlockNumber {
    let mut new_boundary = current_boundary;
    let mut last_scanned_block = current_boundary;

    let blocks_iter: Box<dyn Iterator<Item = &BlockNumber>> = match direction {
        ScanDirection::Backward => Box::new(cold.range(..current_boundary).rev()),
        ScanDirection::Forward => {
            let start = current_boundary.saturating_add(1);
            Box::new(cold.range(start..))
        }
    };

    for &block in blocks_iter {
        // Stop if this cold block is also hot
        if hot.contains(&block) {
            break;
        }

        // Check for hot blocks in the gap between last_scanned_block and current block
        let hot_barrier = match direction {
            ScanDirection::Backward => {
                // Gap is (block + 1)..last_scanned_block
                // Find the first (highest) hot block in this range
                hot.range((block + 1)..last_scanned_block).next_back()
            }
            ScanDirection::Forward => {
                // Gap is (last_scanned_block + 1)..block
                // Find the first (lowest) hot block in this range
                hot.range((last_scanned_block + 1)..block).next()
            }
        };

        if let Some(&hot_block) = hot_barrier {
            // Advance cursor to the block right before the hot barrier
            new_boundary = match direction {
                ScanDirection::Backward => hot_block.saturating_add(1),
                ScanDirection::Forward => hot_block.saturating_sub(1),
            };
            break;
        }

        new_boundary = block;
        last_scanned_block = block;
    }

    new_boundary
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn set(blocks: &[BlockNumber]) -> BTreeSet<BlockNumber> {
        blocks.iter().copied().collect()
    }

    #[test]
    fn cursor_blocks_builder_separates_cold_and_hot() {
        let mut touched = HashMap::new();
        touched.insert(1u64, set(&[10, 12, 12, 13]));
        touched.insert(2u64, set(&[7, 8]));

        let mut builder = CursorBlocksBuilder::default();
        builder.merge_cold(5, &touched);

        let mut hot = HashMap::new();
        hot.insert(1u64, set(&[9]));
        builder.merge_hot(5, &hot);

        let sets = builder.inner.get(&(5, 1)).cloned().expect("missing chain1");
        assert_eq!(sets.cold, set(&[10, 12, 13]));
        assert_eq!(sets.hot, set(&[9]));

        let chain2 = builder.inner.get(&(5, 2)).cloned().expect("missing chain2");
        assert_eq!(chain2.cold, set(&[7, 8]));
        assert!(chain2.hot.is_empty());
    }

    #[test]
    fn cursor_blocks_builder_keys_iterator() {
        let mut touched = HashMap::new();
        touched.insert(1u64, set(&[1]));

        let mut builder = CursorBlocksBuilder::default();
        builder.merge_cold(1, &touched);
        builder.merge_hot(2, &touched);

        let keys: BTreeSet<_> = builder.inner.keys().copied().collect();
        assert_eq!(keys, BTreeSet::from([(1, 1u64), (2, 1u64)]));
    }

    #[test]
    fn block_sets_bootstrap_delegates() {
        let sets = BlockSets {
            cold: set(&[1, 5, 10]),
            hot: set(&[7]),
        };

        let cursor = sets.bootstrap_cursor().expect("cursor should exist");
        assert_eq!(cursor.backward, 1);
        assert_eq!(cursor.forward, 5);
    }

    #[test]
    fn block_sets_extend_delegates() {
        let sets = BlockSets {
            cold: set(&[5, 9, 10, 21, 25]),
            hot: set(&[7, 23]),
        };
        let cursor = Cursor {
            backward: 10,
            forward: 20,
        };

        let updated = sets.extend_cursor(cursor);
        assert_eq!(
            updated.backward, 8,
            "extends backward to block right after hot=7"
        );
        assert_eq!(
            updated.forward, 22,
            "extends forward to block right before hot=23"
        );
    }

    struct ExtendBoundaryCase {
        direction: ScanDirection,
        boundary: BlockNumber,
        cold: &'static [BlockNumber],
        hot: &'static [BlockNumber],
        expected: BlockNumber,
    }

    #[rstest]
    #[case::backward_stops_at_direct_hot(ExtendBoundaryCase {
        direction: ScanDirection::Backward,
        boundary: 10,
        cold: &[5, 7, 8, 9, 10],
        hot: &[7],
        expected: 8,
    })]
    #[case::backward_stops_at_hot_in_gap(ExtendBoundaryCase {
        direction: ScanDirection::Backward,
        boundary: 10,
        cold: &[5, 10],
        hot: &[7],
        expected: 8,
    })]
    #[case::forward_extends_through_gaps(ExtendBoundaryCase {
        direction: ScanDirection::Forward,
        boundary: 10,
        cold: &[10, 15, 20],
        hot: &[],
        expected: 20,
    })]
    #[case::forward_stops_at_hot_in_gap(ExtendBoundaryCase {
        direction: ScanDirection::Forward,
        boundary: 10,
        cold: &[10, 20],
        hot: &[15],
        expected: 14,
    })]
    #[case::backward_no_cold_below(ExtendBoundaryCase {
        direction: ScanDirection::Backward,
        boundary: 10,
        cold: &[10, 15],
        hot: &[],
        expected: 10,
    })]
    #[case::forward_no_cold_above(ExtendBoundaryCase {
        direction: ScanDirection::Forward,
        boundary: 20,
        cold: &[10, 15, 20],
        hot: &[],
        expected: 20,
    })]
    #[case::forward_stops_at_direct_hot(ExtendBoundaryCase {
        direction: ScanDirection::Forward,
        boundary: 10,
        cold: &[10, 12, 15, 20],
        hot: &[15],
        expected: 12,
    })]
    #[case::boundary_is_hot_no_effect(ExtendBoundaryCase {
        direction: ScanDirection::Forward,
        boundary: 10,
        cold: &[10, 12, 15],
        hot: &[10],
        expected: 15,
    })]
    fn extend_boundary(
        #[case] ExtendBoundaryCase {
            direction,
            boundary,
            cold,
            hot,
            expected,
        }: ExtendBoundaryCase,
    ) {
        let cold = set(cold);
        let hot = set(hot);

        let updated = extend_cursor_boundary(direction, boundary, &cold, &hot);

        assert_eq!(updated, expected);
    }

    struct ExtendCursorCase {
        backward: BlockNumber,
        forward: BlockNumber,
        cold: &'static [BlockNumber],
        hot: &'static [BlockNumber],
        expected_backward: BlockNumber,
        expected_forward: BlockNumber,
    }

    #[rstest]
    #[case::stops_at_direct_hot(ExtendCursorCase {
        backward: 10,
        forward: 20,
        cold: &[7, 8, 9, 10, 11, 21, 22, 23],
        hot: &[8, 22],
        expected_backward: 9,
        expected_forward: 21,
    })]
    #[case::bridges_all_gaps_no_hot(ExtendCursorCase {
        backward: 10,
        forward: 20,
        cold: &[5, 9, 10, 21, 25],
        hot: &[],
        expected_backward: 5,
        expected_forward: 25,
    })]
    #[case::stops_at_hot_in_gap(ExtendCursorCase {
        backward: 10,
        forward: 20,
        cold: &[5, 9, 10, 21, 25],
        hot: &[7, 23],
        expected_backward: 8,
        expected_forward: 22,
    })]
    #[case::extends_both_no_hot(ExtendCursorCase {
        backward: 10,
        forward: 20,
        cold: &[5, 10, 20, 25],
        hot: &[],
        expected_backward: 5,
        expected_forward: 25,
    })]
    #[case::boundary_is_hot_extends(ExtendCursorCase {
        backward: 10,
        forward: 20,
        cold: &[5, 10, 20, 25],
        hot: &[10, 20],
        expected_backward: 5,
        expected_forward: 25,
    })]
    #[case::advances_to_block_before_hot(ExtendCursorCase {
        backward: 10,
        forward: 10,
        cold: &[10, 20],
        hot: &[15],
        expected_backward: 10,
        expected_forward: 14,
    })]
    fn extend_cursor(
        #[case] ExtendCursorCase {
            backward,
            forward,
            cold,
            hot,
            expected_backward,
            expected_forward,
        }: ExtendCursorCase,
    ) {
        let sets = BlockSets {
            cold: set(cold),
            hot: set(hot),
        };

        // The cursor represents scan coverage, not just event locations. It can
        // advance to blocks in gaps (scanned but empty) and will stop at the
        // block right before a hot barrier.
        let initial = Cursor { backward, forward };
        let updated = sets.extend_cursor(initial);

        assert_eq!(updated.backward, expected_backward);
        assert_eq!(updated.forward, expected_forward);
    }

    struct BootstrapCase {
        cold: &'static [BlockNumber],
        hot: &'static [BlockNumber],
        expected: Option<(BlockNumber, BlockNumber)>,
    }

    #[rstest]
    #[case::longest_range_before_hot(BootstrapCase {
        cold: &[1, 2, 3, 10, 12, 13, 20],
        hot: &[11],
        expected: Some((1, 10)),
    })]
    #[case::bridges_all_gaps_no_hot(BootstrapCase {
        cold: &[1, 5, 10, 20],
        hot: &[],
        expected: Some((1, 20)),
    })]
    #[case::none_when_all_cold_is_hot(BootstrapCase {
        cold: &[5, 6, 7],
        hot: &[5, 6, 7],
        expected: None,
    })]
    #[case::single_cold_block(BootstrapCase {
        cold: &[42],
        hot: &[],
        expected: Some((42, 42)),
    })]
    #[case::empty_cold_set(BootstrapCase {
        cold: &[],
        hot: &[],
        expected: None,
    })]
    #[case::second_range_is_longer(BootstrapCase {
        cold: &[1, 2, 10, 11, 12, 13, 20],
        hot: &[5],
        expected: Some((10, 20)),
    })]
    fn bootstrap(
        #[case] BootstrapCase {
            cold,
            hot,
            expected,
        }: BootstrapCase,
    ) {
        let sets = BlockSets {
            cold: set(cold),
            hot: set(hot),
        };

        let cursor = sets.bootstrap_cursor();

        assert_eq!(
            cursor.map(|value| (value.backward, value.forward)),
            expected
        );
    }
}
