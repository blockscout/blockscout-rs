// SPDX-License-Identifier: LicenseRef-Blockscout

//! Pure, DB-free arithmetic turning stored checkpoint cursors into a
//! reportable catch-up progress snapshot for one `(bridge_id, chain_id)`
//! pair. No DB types, no proto types: this module cannot drift from the
//! `GetIndexingProgress` endpoint's contract, and every branch here is
//! covered by the unit tests below.
//!
//! **What the percentage is not.** [`CatchupProgress::progress_percent`] is
//! the *scanned* share, never a completeness measure. A range that was
//! scanned and then failed downstream processing still counts as scanned,
//! because that is exactly what the cursors record. So 100% with
//! `failed_blocks > 0` (reported by the caller alongside this struct) is a
//! normal, correct reading — `failed_blocks` is the only completeness
//! signal in the payload.

/// Stored checkpoint cursors for one `(bridge_id, chain_id)`, already clamped
/// to `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointCursors {
    pub catchup_min_cursor: u64,
    pub catchup_max_cursor: u64,
    pub realtime_cursor: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CatchupProgress {
    pub start_block: u64,
    pub catchup_min_cursor: u64,
    pub catchup_max_cursor: u64,
    pub realtime_cursor: u64,
    pub scan_complete: bool,
    pub progress_percent: f64,
    pub blocks_remaining: u64,
}

impl CatchupProgress {
    /// `cursors == None` means no checkpoint row exists yet: report zeroed
    /// cursors, `blocks_remaining = 0`, `scan_complete = false` (absence is
    /// *unknown*, not "done"), and `progress_percent = 0.0`.
    ///
    /// With `S = start_block`, `M = catchup_min_cursor`, `X =
    /// catchup_max_cursor`, `R = realtime_cursor`:
    ///
    /// ```text
    /// lo               = max(M, S)                                   // read-side guard
    /// blocks_remaining = if X < lo { 0 } else { X - lo + 1 }          // size of the unscanned interval
    /// scan_complete    = cursors.is_some() && blocks_remaining == 0
    /// total            = if R < S { 0 } else { R - S + 1 }
    /// progress_percent = if total == 0 { 0.0 }
    ///                    else { 100.0 * (total - min(blocks_remaining, total)) as f64 / total as f64 }
    ///                    then clamped to [0.0, 100.0]
    /// ```
    ///
    /// **Why each clamp exists — none of these is defensive padding:**
    ///
    /// - `lo = max(M, S)` is the read-side guard. A row not yet healed by the
    ///   startup seed stores `M = 0`, which would otherwise inflate the
    ///   unscanned count all the way down to block 0.
    /// - `scan_complete` is derived from `X < lo`, not from `X < M`. This is
    ///   load-bearing. The row `mark_catchup_complete` actually writes is
    ///   `X = S - 1`, so an un-healed row is `(M = 0, X = S - 1)` and `X < M`
    ///   is **false** — the naive predicate would report a finished catch-up
    ///   as unfinished forever. Applying the guard first fixes it, and the
    ///   guarded form is identical to "the two cursors met" whenever the
    ///   floor is healed, including the future bidirectional case where
    ///   `M > S`.
    /// - `cursors.is_some() &&`: with no row, `blocks_remaining` is *unknown*,
    ///   not zero. Reporting `scan_complete = false` is the honest answer;
    ///   absence is signalled by a missing `checkpoint_updated_at` at the
    ///   caller.
    /// - `min(blocks_remaining, total)` guards `X > R`, which the cursor
    ///   algebra forbids in normal operation but a hand-edited row does not.
    /// - The `total == 0` short-circuit: `R < S` is a misconfiguration —
    ///   clamp to `0` and do not divide. This also keeps `progress_percent`
    ///   **finite**, which matters more than it looks: `serde_json` renders a
    ///   non-finite `f64` as `null`, so a NaN would silently turn a
    ///   non-optional response field into `null`.
    pub fn compute(start_block: u64, cursors: Option<CheckpointCursors>) -> Self {
        let Some(cursors) = cursors else {
            return Self {
                start_block,
                catchup_min_cursor: 0,
                catchup_max_cursor: 0,
                realtime_cursor: 0,
                scan_complete: false,
                progress_percent: 0.0,
                blocks_remaining: 0,
            };
        };

        let CheckpointCursors {
            catchup_min_cursor,
            catchup_max_cursor,
            realtime_cursor,
        } = cursors;

        // Read-side guard: a stored floor not yet healed by the startup seed
        // must not inflate the unscanned interval down to block 0.
        let lo = catchup_min_cursor.max(start_block);

        let blocks_remaining = if catchup_max_cursor < lo {
            0
        } else {
            // `catchup_max_cursor >= lo` here, so this cannot underflow.
            catchup_max_cursor - lo + 1
        };
        let scan_complete = blocks_remaining == 0;

        let total = if realtime_cursor < start_block {
            0
        } else {
            realtime_cursor - start_block + 1
        };

        let progress_percent = if total == 0 {
            0.0
        } else {
            let remaining = blocks_remaining.min(total);
            let scanned = total.saturating_sub(remaining);
            (100.0 * scanned as f64 / total as f64).clamp(0.0, 100.0)
        };

        Self {
            start_block,
            catchup_min_cursor,
            catchup_max_cursor,
            realtime_cursor,
            scan_complete,
            progress_percent,
            blocks_remaining,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    fn cursors(
        catchup_min_cursor: u64,
        catchup_max_cursor: u64,
        realtime_cursor: u64,
    ) -> CheckpointCursors {
        CheckpointCursors {
            catchup_min_cursor,
            catchup_max_cursor,
            realtime_cursor,
        }
    }

    #[test]
    fn test_compute_completion_boundary_x_equals_lo_minus_one_reads_complete() {
        // S = 100, M = 0 (un-healed) => lo = 100. X = 99 = lo - 1.
        let progress = CatchupProgress::compute(100, Some(cursors(0, 99, 200)));
        assert!(progress.scan_complete);
        assert_eq!(progress.blocks_remaining, 0);
    }

    #[test]
    fn test_compute_completion_boundary_x_equals_lo_does_not_read_complete() {
        let progress = CatchupProgress::compute(100, Some(cursors(0, 100, 200)));
        assert!(!progress.scan_complete);
        assert_eq!(progress.blocks_remaining, 1);
    }

    #[rstest]
    #[case(100, 150, 200, 51)] // blocks_remaining = X - lo + 1 = 150 - 100 + 1
    #[case(100, 100, 200, 1)]
    #[case(100, 300, 500, 201)]
    fn test_compute_blocks_remaining_equals_interval_width(
        #[case] start_block: u64,
        #[case] catchup_max_cursor: u64,
        #[case] realtime_cursor: u64,
        #[case] expected_remaining: u64,
    ) {
        let progress = CatchupProgress::compute(
            start_block,
            Some(cursors(0, catchup_max_cursor, realtime_cursor)),
        );
        assert_eq!(progress.blocks_remaining, expected_remaining);
    }

    #[test]
    fn test_compute_blocks_remaining_is_zero_once_complete() {
        let progress = CatchupProgress::compute(100, Some(cursors(0, 99, 200)));
        assert_eq!(progress.blocks_remaining, 0);
    }

    #[test]
    fn test_compute_completion_derived_from_cursors_meeting_future_bidirectional_case() {
        // M advanced above S: the future bidirectional case. lo = max(M, S) = M.
        // X = M - 1 means the two cursors have met.
        let progress = CatchupProgress::compute(100, Some(cursors(150, 149, 200)));
        assert!(progress.scan_complete);
        assert_eq!(progress.blocks_remaining, 0);
    }

    #[test]
    fn test_compute_unhealed_post_completion_row_reads_complete() {
        // The un-healed post-completion row the naive `X < M` predicate gets
        // wrong: M = 0, X = S - 1. `X < M` is false, but the guarded form
        // (`X < max(M, S)`) correctly reads this as complete.
        let progress = CatchupProgress::compute(1000, Some(cursors(0, 999, 2000)));
        assert!(progress.scan_complete);
        assert_eq!(progress.blocks_remaining, 0);
    }

    #[test]
    fn test_compute_read_side_guard_treats_legacy_zero_min_cursor_as_start_block() {
        // A legacy M == 0 must not inflate the remainder all the way to block 0.
        let progress = CatchupProgress::compute(1000, Some(cursors(0, 1500, 2000)));
        assert_eq!(progress.blocks_remaining, 501); // 1500 - 1000 + 1, not 1500 - 0 + 1
    }

    #[test]
    fn test_compute_backward_compatible_with_one_directional_formula_when_m_equals_s() {
        let start_block = 100u64;
        let catchup_max_cursor = 150u64;
        let realtime_cursor = 500u64;
        let progress = CatchupProgress::compute(
            start_block,
            Some(cursors(start_block, catchup_max_cursor, realtime_cursor)),
        );

        let total = realtime_cursor - start_block + 1;
        let remaining = catchup_max_cursor - start_block + 1;
        let expected = 100.0 * (total - remaining) as f64 / total as f64;

        assert!((progress.progress_percent - expected).abs() < 1e-9);
    }

    #[test]
    fn test_compute_monotonicity_shrinking_interval_never_decreases_percentage() {
        let start_block = 100u64;
        let realtime_cursor = 1000u64;

        let wide = CatchupProgress::compute(start_block, Some(cursors(0, 900, realtime_cursor)));
        let narrower_from_min =
            CatchupProgress::compute(start_block, Some(cursors(200, 900, realtime_cursor)));
        let narrower_from_max =
            CatchupProgress::compute(start_block, Some(cursors(0, 700, realtime_cursor)));

        assert!(narrower_from_min.progress_percent >= wide.progress_percent);
        assert!(narrower_from_max.progress_percent >= wide.progress_percent);
    }

    #[test]
    fn test_compute_monotonicity_increasing_realtime_cursor_never_decreases_percentage() {
        let start_block = 100u64;
        let before = CatchupProgress::compute(start_block, Some(cursors(0, 900, 1000)));
        let after = CatchupProgress::compute(start_block, Some(cursors(0, 900, 2000)));

        assert!(after.progress_percent >= before.progress_percent);
    }

    #[test]
    fn test_compute_degenerate_realtime_below_start_block_reports_zero_without_panic() {
        let progress = CatchupProgress::compute(1000, Some(cursors(0, 500, 200)));
        assert_eq!(progress.progress_percent, 0.0);
    }

    #[test]
    fn test_compute_hand_edited_row_with_max_above_realtime_clamps_percentage_not_blocks_remaining()
    {
        // X > R, which the cursor algebra forbids in normal operation but a
        // hand-edited row does not. Only the value feeding the percentage is
        // clamped (`min(blocks_remaining, total)`); `blocks_remaining` itself
        // stays the true, unclamped interval width `X - lo + 1`.
        let start_block = 100u64;
        let catchup_max_cursor = 5000u64;
        let realtime_cursor = 200u64;
        let progress = CatchupProgress::compute(
            start_block,
            Some(cursors(0, catchup_max_cursor, realtime_cursor)),
        );

        // M = 0 is un-healed here, so lo = max(M, S) = S.
        let lo = start_block;
        assert_eq!(
            progress.blocks_remaining,
            catchup_max_cursor - lo + 1,
            "blocks_remaining must report the true unscanned width, not a value clamped to realtime_cursor"
        );
        assert!((0.0..=100.0).contains(&progress.progress_percent));
    }

    #[test]
    fn test_compute_no_checkpoint_row_reports_zero_and_incomplete() {
        let progress = CatchupProgress::compute(1000, None);
        assert_eq!(progress.progress_percent, 0.0);
        assert!(!progress.scan_complete);
        assert_eq!(progress.blocks_remaining, 0);
        assert_eq!(progress.catchup_min_cursor, 0);
        assert_eq!(progress.catchup_max_cursor, 0);
        assert_eq!(progress.realtime_cursor, 0);
    }

    #[rstest]
    #[case(0, None)]
    #[case(1000, Some(cursors(0, 999, 2000)))]
    #[case(1000, Some(cursors(0, 5000, 200)))]
    #[case(1000, Some(cursors(0, 500, 200)))]
    #[case(100, Some(cursors(150, 149, 200)))]
    fn test_compute_result_is_always_finite_and_within_bounds(
        #[case] start_block: u64,
        #[case] cursors: Option<CheckpointCursors>,
    ) {
        let progress = CatchupProgress::compute(start_block, cursors);
        assert!(progress.progress_percent.is_finite());
        assert!((0.0..=100.0).contains(&progress.progress_percent));
    }
}
