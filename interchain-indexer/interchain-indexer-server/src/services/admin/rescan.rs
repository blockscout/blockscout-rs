// SPDX-License-Identifier: LicenseRef-Blockscout

//! `RescanBlockRanges`: validation, planning and the replay-time estimate.
//! Pure functions: they never touch the database.
//!
//! A request is validated in two phases. Phase 1 looks at the request and the
//! configuration only (`INVALID_ARGUMENT`). Phase 2 looks at a snapshot of the
//! checkpoints and of the open failed ranges (`FAILED_PRECONDITION`). Each phase
//! reports every violation it finds, in one error.

use super::error::AdminError;
use crate::{
    BridgeConfig, Settings, config::IndexerType, indexers::IndexingTarget, proto::RescanBlockRange,
};
use interchain_indexer_entity::{indexer_checkpoints, sea_orm_active_enums::BridgeType};
use interchain_indexer_logic::indexer::failure_ledger::{
    BlockRange, FailedInterval, FailureRetrySettings, fold_adjacent, overlaps, overlaps_or_adjacent,
};
use std::{
    collections::{BTreeMap, HashMap},
    fmt::Display,
};

pub(crate) const MAX_RESCAN_RANGES_PER_REQUEST: usize = 100;
/// Blocks in one request after merging: about 4 hours of replay on an Avalanche
/// or AMB bridge (`batch_size` 1000) and about 8 on xDai (500).
pub(crate) const MAX_RESCAN_BLOCKS_PER_REQUEST: u64 = 2_000_000;
/// Same bound as the driver's own `indexer_failures.reason` cap.
pub(crate) const RESCAN_LEDGER_REASON_MAX_BYTES: usize = 500;

/// The highest block number the ledger can store (`BIGINT`).
const MAX_BLOCK_NUMBER: u64 = i64::MAX.unsigned_abs();

/// What a bridge's indexer replays failed ranges with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReplayProfile {
    pub batch_size: u64,
    pub failure_retry: FailureRetrySettings,
}

/// Config-only: the same `(bridge_type, indexer_type)` -> settings mapping as
/// `spawn_configured_indexers`. Enabled bridges only; other combinations get no
/// entry.
pub(crate) fn build_replay_profiles(
    bridges: &[BridgeConfig],
    settings: &Settings,
) -> BTreeMap<i32, ReplayProfile> {
    bridges
        .iter()
        .filter(|bridge| bridge.enabled)
        .filter_map(|bridge| {
            let (batch_size, failure_retry) = match (&bridge.bridge_type, bridge.indexer_type) {
                (BridgeType::AvalancheNative, IndexerType::IcmIctt) => (
                    settings.avalanche_indexer.batch_size,
                    &settings.avalanche_indexer.failure_retry,
                ),
                (BridgeType::Amb, IndexerType::AMB) => (
                    settings.amb_indexer.batch_size,
                    &settings.amb_indexer.failure_retry,
                ),
                (BridgeType::Xdai, IndexerType::XDai) => (
                    settings.xdai_indexer.batch_size,
                    &settings.xdai_indexer.failure_retry,
                ),
                _ => return None,
            };
            let profile = ReplayProfile {
                batch_size,
                failure_retry: failure_retry.clone(),
            };
            Some((bridge.bridge_id, profile))
        })
        .collect()
}

pub(crate) struct RescanContext<'a> {
    pub targets: &'a [IndexingTarget],
    pub profiles: &'a BTreeMap<i32, ReplayProfile>,
}

pub(crate) struct RescanPlan {
    /// Normalized ranges of the request, sorted by `(bridge_id, chain_id, from)`.
    pub scheduled: Vec<(i32, i64, BlockRange)>,
    pub requested_blocks: u64,
    /// `(bridge_id, estimated_drain_seconds)`, sorted by bridge.
    pub estimates: Vec<(i32, u64)>,
}

fn width(range: BlockRange) -> u64 {
    range.to.saturating_sub(range.from).saturating_add(1)
}

fn describe(bridge_id: i32, chain_id: i64, range: BlockRange) -> String {
    format!(
        "bridge {bridge_id}, chain {chain_id}, {}..{}",
        range.from, range.to
    )
}

/// Phase 1 (static, `INVALID_ARGUMENT`): checks each element against the
/// request limits and the configuration, and returns the ranges folded per pair
/// (overlapping and adjacent ones merged), sorted by `(bridge, chain, from)`.
///
/// A request that breaks the range-count limit is rejected on that alone: its
/// per-element messages would not be bounded.
pub(crate) fn validate_rescan_input(
    ranges: &[RescanBlockRange],
    ctx: &RescanContext,
) -> Result<Vec<(i32, i64, BlockRange)>, AdminError> {
    if ranges.is_empty() {
        Err(AdminError::InvalidArgument(
            "ranges must not be empty".to_string(),
        ))?;
    }
    if ranges.len() > MAX_RESCAN_RANGES_PER_REQUEST {
        Err(AdminError::InvalidArgument(format!(
            "too many ranges: {} > {MAX_RESCAN_RANGES_PER_REQUEST}",
            ranges.len()
        )))?;
    }

    let mut violations = Vec::new();
    let mut accepted = Vec::new();
    for (index, item) in ranges.iter().enumerate() {
        let problems = element_problems(item, ctx);
        if problems.is_empty() {
            accepted.push((
                item.bridge_id,
                item.chain_id,
                BlockRange {
                    from: item.from_block,
                    to: item.to_block,
                },
            ));
        } else {
            violations.extend(problems.into_iter().map(|problem| {
                format!(
                    "ranges[{index}] (bridge {}, chain {}, {}..{}): {problem}",
                    item.bridge_id, item.chain_id, item.from_block, item.to_block
                )
            }));
        }
    }

    // Only the elements that would otherwise be accepted count towards the
    // total, so the reported figure is the one a corrected request would have.
    let folded = fold_per_pair(accepted);
    let total_blocks = folded.iter().fold(0u64, |total, (_, _, range)| {
        total.saturating_add(width(*range))
    });
    if total_blocks > MAX_RESCAN_BLOCKS_PER_REQUEST {
        violations.push(format!(
            "too many blocks after merging: {total_blocks} > {MAX_RESCAN_BLOCKS_PER_REQUEST}"
        ));
    }

    match violations.is_empty() {
        true => Ok(folded),
        false => Err(AdminError::InvalidArgument(violations.join("; "))),
    }
}

/// Checks 2-4 for one element of the request.
fn element_problems(item: &RescanBlockRange, ctx: &RescanContext) -> Vec<String> {
    let mut problems = Vec::new();
    if item.from_block > item.to_block {
        problems.push("from_block must not exceed to_block".to_string());
    }
    if item.to_block > MAX_BLOCK_NUMBER {
        problems.push(format!("to_block must be at most {MAX_BLOCK_NUMBER}"));
    }
    let target = ctx
        .targets
        .iter()
        .find(|target| target.bridge_id == item.bridge_id && target.chain_id == item.chain_id);
    match target {
        None => problems.push("pair is not a configured, enabled indexing target".to_string()),
        Some(target) if item.from_block < target.start_block => problems.push(format!(
            "from_block is below the scan floor {}",
            target.start_block
        )),
        Some(_) => {}
    }
    problems
}

/// Folds each pair's ranges together; the result is sorted by
/// `(bridge, chain, from)`.
fn fold_per_pair(
    ranges: impl IntoIterator<Item = (i32, i64, BlockRange)>,
) -> Vec<(i32, i64, BlockRange)> {
    let mut by_pair: BTreeMap<(i32, i64), Vec<(BlockRange, ())>> = BTreeMap::new();
    for (bridge_id, chain_id, range) in ranges {
        by_pair
            .entry((bridge_id, chain_id))
            .or_default()
            .push((range, ()));
    }
    by_pair
        .into_iter()
        .flat_map(|((bridge_id, chain_id), ranges)| {
            fold_adjacent(ranges)
                .into_iter()
                .map(move |(range, ())| (bridge_id, chain_id, range))
        })
        .collect()
}

/// Phase 2 (state, `FAILED_PRECONDITION`): checks the normalized ranges against
/// the replay profiles, the checkpoints and the open failed ranges, and
/// estimates the replay time per bridge.
///
/// Violations are described by the normalized range, never by an index of the
/// request: after folding and sorting an element no longer has one.
pub(crate) fn plan_rescan(
    folded: &[(i32, i64, BlockRange)],
    ctx: &RescanContext,
    checkpoints: &HashMap<(i32, i64), indexer_checkpoints::Model>,
    open: &[(i32, i64, FailedInterval)],
) -> Result<RescanPlan, AdminError> {
    let mut violations = Vec::new();
    // Per bridge: its profile and the replay chunks of this request.
    let mut chunks_by_bridge: BTreeMap<i32, (&ReplayProfile, u64)> = BTreeMap::new();

    for &(bridge_id, chain_id, range) in folded {
        let mut problems = Vec::new();

        match ctx.profiles.get(&bridge_id) {
            Some(profile) if profile.failure_retry.enabled => {
                let (_, chunks) = chunks_by_bridge.entry(bridge_id).or_insert((profile, 0));
                *chunks = chunks.saturating_add(width(range).div_ceil(profile.batch_size.max(1)));
            }
            _ => problems.push(
                "failed-range replay is unavailable for this bridge (no indexer for its type, \
                 or failure_retry.enabled = false)"
                    .to_string(),
            ),
        }

        let start_block = ctx
            .targets
            .iter()
            .find(|target| target.bridge_id == bridge_id && target.chain_id == chain_id)
            // Phase 1 guarantees the target. Without one, a floor of 0 widens
            // the unscanned interval, so the check can only get stricter.
            .map_or(0, |target| target.start_block);
        problems.extend(checkpoint_problems(
            range,
            start_block,
            checkpoints.get(&(bridge_id, chain_id)),
        ));

        // Rows are described by chain and bounds only: their `reason` carries
        // provider error text, and attempts and timestamps are internal.
        let conflicts: Vec<String> = open
            .iter()
            .filter(|(open_bridge, open_chain, row)| {
                *open_bridge == bridge_id
                    && *open_chain == chain_id
                    && overlaps_or_adjacent(range, row.range)
            })
            .map(|(_, open_chain, row)| {
                format!("chain {open_chain} {}..{}", row.range.from, row.range.to)
            })
            .collect();
        if !conflicts.is_empty() {
            problems.push(format!(
                "overlaps or touches open failed range(s): {} (wait until they are replayed)",
                conflicts.join(", ")
            ));
        }

        violations.extend(
            problems
                .into_iter()
                .map(|problem| format!("{}: {problem}", describe(bridge_id, chain_id, range))),
        );
    }

    if !violations.is_empty() {
        Err(AdminError::FailedPrecondition(violations.join("; ")))?;
    }

    Ok(RescanPlan {
        scheduled: folded.to_vec(),
        requested_blocks: folded.iter().fold(0u64, |total, (_, _, range)| {
            total.saturating_add(width(*range))
        }),
        estimates: chunks_by_bridge
            .into_iter()
            .map(|(bridge_id, (profile, chunks))| {
                (bridge_id, estimated_drain_seconds(profile, chunks))
            })
            .collect(),
    })
}

/// Checks 7 and 8: the range must lie in scanned territory that no scan of the
/// running indexer still owns.
fn checkpoint_problems(
    range: BlockRange,
    start_block: u64,
    checkpoint: Option<&indexer_checkpoints::Model>,
) -> Vec<String> {
    let Some(checkpoint) = checkpoint else {
        return vec!["no checkpoint yet".to_string()];
    };

    let mut problems = Vec::new();
    // The realtime cursor is the NEXT block to scan.
    match checkpoint.validated_realtime_cursor().checked_sub(1) {
        None => problems.push("no checkpoint yet: nothing has been scanned".to_string()),
        Some(last_scanned) if range.to > last_scanned => {
            problems.push(format!("above the last scanned block {last_scanned}"))
        }
        Some(_) => {}
    }

    // The unscanned catch-up interval, with the same arithmetic as
    // `CatchupProgress::compute`.
    let lo = checkpoint.validated_catchup_min_cursor().max(start_block);
    let hi = checkpoint.validated_catchup_cursor();
    if hi >= lo && overlaps(range, BlockRange { from: lo, to: hi }) {
        problems.push(format!(
            "overlaps the unscanned catch-up interval {lo}..{hi} (catch-up will process it)"
        ));
    }
    problems
}

/// An estimate, not a bound: the replay budget (`max_chunks_per_pass`) is shared
/// with the bridge's real failed ranges, and chunks may fail or narrow.
///
/// ```text
/// ticks = ceil(chunks / max(max_chunks_per_pass, 1))
/// eta   = backoff_base + ticks * scan_interval
/// ```
///
/// The divisors are clamped to 1 because an indexer that failed to construct
/// leaves unvalidated settings behind.
fn estimated_drain_seconds(profile: &ReplayProfile, chunks: u64) -> u64 {
    let settings = &profile.failure_retry;
    let chunks_per_pass = u64::try_from(settings.max_chunks_per_pass)
        .unwrap_or(u64::MAX)
        .max(1);
    let ticks = chunks.div_ceil(chunks_per_pass);
    settings
        .backoff_base
        .as_secs()
        .saturating_add(ticks.saturating_mul(settings.scan_interval.as_secs()))
}

/// The ledger `reason` of a queued row. A reference only: a later merge of the
/// row overwrites it, and the audit log is the record.
pub(crate) fn ledger_reason(actor: impl Display, reason: &str) -> String {
    let full = format!("write-api: {actor}: {reason}");
    truncate_to_char_boundary(&full, RESCAN_LEDGER_REASON_MAX_BYTES).to_string()
}

fn truncate_to_char_boundary(text: &str, max_bytes: usize) -> &str {
    let end = (0..=max_bytes.min(text.len()))
        .rev()
        .find(|&end| text.is_char_boundary(end))
        .unwrap_or(0);
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tonic::Code;

    const FLOOR: u64 = 1000;

    /// Bridge 1 (chains 1 and 100, floors 1000 and 2000), bridge 2 (no profile),
    /// bridge 3 (replay disabled), bridge 4 (a different batch size).
    struct Env {
        targets: Vec<IndexingTarget>,
        profiles: BTreeMap<i32, ReplayProfile>,
    }

    fn retry_settings(enabled: bool) -> FailureRetrySettings {
        FailureRetrySettings {
            enabled,
            scan_interval: Duration::from_secs(60),
            backoff_base: Duration::from_secs(30),
            max_chunks_per_pass: 8,
            ..Default::default()
        }
    }

    fn env() -> Env {
        let target = |bridge_id, chain_id, start_block| IndexingTarget {
            bridge_id,
            chain_id,
            start_block,
        };
        let profile = |batch_size, enabled| ReplayProfile {
            batch_size,
            failure_retry: retry_settings(enabled),
        };
        Env {
            targets: vec![
                target(1, 1, FLOOR),
                target(1, 100, 2000),
                target(2, 1, 0),
                target(3, 1, 0),
                target(4, 1, 0),
            ],
            profiles: BTreeMap::from([(1, profile(1000, true)), (3, profile(1000, false))]),
        }
    }

    impl Env {
        fn ctx(&self) -> RescanContext<'_> {
            RescanContext {
                targets: &self.targets,
                profiles: &self.profiles,
            }
        }
    }

    fn item(bridge_id: i32, chain_id: i64, from_block: u64, to_block: u64) -> RescanBlockRange {
        RescanBlockRange {
            bridge_id,
            chain_id,
            from_block,
            to_block,
        }
    }

    fn range(from: u64, to: u64) -> BlockRange {
        BlockRange { from, to }
    }

    /// A checkpoint with the given `(M, X, R)` cursors.
    fn checkpoint(
        bridge_id: i32,
        chain_id: i64,
        catchup_min: i64,
        catchup_max: i64,
        realtime: i64,
    ) -> ((i32, i64), indexer_checkpoints::Model) {
        (
            (bridge_id, chain_id),
            indexer_checkpoints::Model {
                bridge_id,
                chain_id,
                catchup_min_cursor: catchup_min,
                catchup_max_cursor: catchup_max,
                finality_cursor: 0,
                realtime_cursor: realtime,
                created_at: None,
                updated_at: None,
            },
        )
    }

    /// Every pair of the fixture fully scanned (catch-up done) up to block
    /// `realtime - 1`.
    fn checkpoints_scanned_to(realtime: i64) -> HashMap<(i32, i64), indexer_checkpoints::Model> {
        HashMap::from([
            checkpoint(1, 1, 1000, 999, realtime),
            checkpoint(1, 100, 2000, 1999, realtime),
            checkpoint(2, 1, 0, 0, realtime),
            checkpoint(3, 1, 0, 0, realtime),
            checkpoint(4, 1, 0, 0, realtime),
        ])
    }

    /// Scanned up to block 99 999.
    fn scanned_checkpoints() -> HashMap<(i32, i64), indexer_checkpoints::Model> {
        checkpoints_scanned_to(100_000)
    }

    fn open_row(bridge_id: i32, chain_id: i64, from: u64, to: u64) -> (i32, i64, FailedInterval) {
        let at = NaiveDate::from_ymd_opt(2026, 3, 14)
            .unwrap()
            .and_hms_opt(15, 9, 26)
            .unwrap();
        (
            bridge_id,
            chain_id,
            FailedInterval {
                range: range(from, to),
                attempts: 4242,
                reason: Some("provider said secret-token".to_string()),
                first_failed_at: at,
                last_attempt_at: at,
            },
        )
    }

    /// Both phases, as the handler runs them.
    fn plan_with(
        env: &Env,
        ranges: &[RescanBlockRange],
        checkpoints: &HashMap<(i32, i64), indexer_checkpoints::Model>,
        open: &[(i32, i64, FailedInterval)],
    ) -> Result<RescanPlan, AdminError> {
        let ctx = env.ctx();
        let folded = validate_rescan_input(ranges, &ctx)?;
        plan_rescan(&folded, &ctx, checkpoints, open)
    }

    fn plan(ranges: &[RescanBlockRange]) -> Result<RescanPlan, AdminError> {
        plan_with(&env(), ranges, &scanned_checkpoints(), &[])
    }

    /// The gRPC code and message a client would see.
    fn rejection(result: Result<RescanPlan, AdminError>) -> (Code, String) {
        let status = tonic::Status::from(result.err().expect("the request must be rejected"));
        (status.code(), status.message().to_string())
    }

    fn assert_invalid_argument(
        result: Result<RescanPlan, AdminError>,
        expected_parts: &[&str],
    ) -> String {
        let (code, message) = rejection(result);
        assert_eq!(code, Code::InvalidArgument, "{message}");
        for part in expected_parts {
            assert!(message.contains(part), "`{part}` missing from: {message}");
        }
        message
    }

    fn assert_failed_precondition(
        result: Result<RescanPlan, AdminError>,
        expected_parts: &[&str],
    ) -> String {
        let (code, message) = rejection(result);
        assert_eq!(code, Code::FailedPrecondition, "{message}");
        assert!(!message.contains("ranges["), "{message}");
        for part in expected_parts {
            assert!(message.contains(part), "`{part}` missing from: {message}");
        }
        message
    }

    #[test]
    fn rescan_plan_rejects_empty_and_too_many_ranges() {
        assert_invalid_argument(plan(&[]), &["ranges must not be empty"]);

        let at_limit: Vec<_> = (0..100)
            .map(|index| item(1, 1, 2000 + index * 10, 2005 + index * 10))
            .collect();
        plan(&at_limit).expect("exactly the limit is accepted");

        let too_many: Vec<_> = (0..101)
            .map(|index| item(1, 1, 2000 + index * 10, 2005 + index * 10))
            .collect();
        let message = assert_invalid_argument(plan(&too_many), &["too many ranges: 101 > 100"]);
        assert!(
            !message.contains("ranges["),
            "a limit has no index: {message}"
        );
    }

    #[test]
    fn rescan_plan_rejects_inverted_and_oversized_bounds() {
        let oversized_to = i64::MAX as u64 + 1;
        let message = assert_invalid_argument(
            plan(&[
                item(1, 1, 2000, 1500),
                item(1, 1, 2000, oversized_to),
                item(1, 1, 2000, 2100),
            ]),
            &[
                "ranges[0] (bridge 1, chain 1, 2000..1500): from_block must not exceed to_block",
                "ranges[1] (bridge 1, chain 1, 2000..9223372036854775808): to_block must be at most 9223372036854775807",
            ],
        );
        assert!(!message.contains("ranges[2]"), "{message}");

        plan(&[item(1, 1, 2000, 2000)]).expect("a one-block range is valid");
    }

    #[test]
    fn rescan_plan_rejects_unknown_pair() {
        let message = assert_invalid_argument(
            plan(&[
                item(1, 7, 2000, 2100),
                item(9, 1, 2000, 2100),
                item(1, 1, 2000, 2100),
            ]),
            &[
                "ranges[0] (bridge 1, chain 7, 2000..2100): pair is not a configured, enabled indexing target",
                "ranges[1] (bridge 9, chain 1, 2000..2100): pair is not a configured, enabled indexing target",
            ],
        );
        assert!(!message.contains("ranges[2]"), "{message}");
    }

    #[test]
    fn rescan_plan_rejects_range_below_scan_floor() {
        assert_invalid_argument(
            plan(&[item(1, 1, FLOOR - 1, 2000)]),
            &["ranges[0] (bridge 1, chain 1, 999..2000): from_block is below the scan floor 1000"],
        );
        plan(&[item(1, 1, FLOOR, 2000)]).expect("the floor itself is scannable");
    }

    #[test]
    fn rescan_plan_rejects_total_blocks_over_limit_after_folding() {
        // Two adjacent ranges that fold into 2 000 001 blocks.
        let message = assert_invalid_argument(
            plan(&[
                item(1, 1, 5000, 1_005_000),
                item(1, 1, 1_005_001, 2_005_000),
            ]),
            &["too many blocks after merging: 2000001 > 2000000"],
        );
        assert!(
            !message.contains("ranges["),
            "a limit has no index: {message}"
        );

        // The limit itself is accepted ...
        let env = env();
        let checkpoints = checkpoints_scanned_to(10_000_000);
        let at_limit = plan_with(&env, &[item(1, 1, 5000, 2_004_999)], &checkpoints, &[])
            .expect("2 000 000 blocks");
        assert_eq!(at_limit.requested_blocks, 2_000_000);
        // ... and blocks covered twice count once.
        let overlapping = plan_with(
            &env,
            &[
                item(1, 1, 5000, 1_505_000),
                item(1, 1, 1_000_000, 1_900_000),
            ],
            &checkpoints,
            &[],
        )
        .expect("the union is 5000..1900000");
        assert_eq!(overlapping.requested_blocks, 1_895_001);
    }

    #[test]
    fn rescan_plan_rejects_bridge_without_profile_or_disabled_retry() {
        let env = env();
        let checkpoints = scanned_checkpoints();
        let result = plan_with(
            &env,
            &[item(2, 1, 2000, 2100), item(3, 1, 3000, 3100)],
            &checkpoints,
            &[],
        );
        let message = assert_failed_precondition(
            result,
            &[
                "bridge 2, chain 1, 2000..2100: failed-range replay is unavailable for this bridge",
                "bridge 3, chain 1, 3000..3100: failed-range replay is unavailable for this bridge",
            ],
        );
        assert!(
            message.contains("failure_retry.enabled = false"),
            "{message}"
        );

        plan_with(&env, &[item(1, 1, 2000, 2100)], &checkpoints, &[])
            .expect("bridge 1 has a profile with replay enabled");
    }

    #[test]
    fn rescan_plan_rejects_missing_checkpoint_and_unscanned_head() {
        let env = env();

        // No checkpoint row at all.
        assert_failed_precondition(
            plan_with(&env, &[item(1, 1, 2000, 2100)], &HashMap::new(), &[]),
            &["bridge 1, chain 1, 2000..2100: no checkpoint yet"],
        );

        // A row with nothing scanned (realtime cursor 0).
        let nothing_scanned = HashMap::from([checkpoint(1, 1, 1000, 999, 0)]);
        assert_failed_precondition(
            plan_with(&env, &[item(1, 1, 2000, 2100)], &nothing_scanned, &[]),
            &["no checkpoint yet: nothing has been scanned"],
        );

        // R = 100 000 is the NEXT block to scan, so 99 999 is the last scanned.
        let checkpoints = scanned_checkpoints();
        assert_failed_precondition(
            plan_with(&env, &[item(1, 1, 99_000, 100_000)], &checkpoints, &[]),
            &["bridge 1, chain 1, 99000..100000: above the last scanned block 99999"],
        );
        plan_with(&env, &[item(1, 1, 99_000, 99_999)], &checkpoints, &[])
            .expect("the last scanned block is allowed");
    }

    #[test]
    fn rescan_plan_rejects_unscanned_catchup_interval() {
        let env = env();
        // Catch-up still has [1000, 5000] to scan backwards.
        let catching_up = HashMap::from([checkpoint(1, 1, 1000, 5000, 100_000)]);

        for (from, to) in [(4000, 4100), (1000, 1000), (5000, 5000), (1000, 6000)] {
            assert_failed_precondition(
                plan_with(&env, &[item(1, 1, from, to)], &catching_up, &[]),
                &["overlaps the unscanned catch-up interval 1000..5000 (catch-up will process it)"],
            );
        }
        plan_with(&env, &[item(1, 1, 5001, 5100)], &catching_up, &[])
            .expect("just above the catch-up interval");

        // A floor not yet healed in the checkpoint (M = 0) is read through the
        // configured start block, exactly as the status endpoint does.
        let unhealed = HashMap::from([checkpoint(1, 1, 0, 999, 100_000)]);
        plan_with(&env, &[item(1, 1, 2000, 2100)], &unhealed, &[])
            .expect("X < max(M, start_block): catch-up is complete");
        let unhealed_busy = HashMap::from([checkpoint(1, 1, 0, 3000, 100_000)]);
        assert_failed_precondition(
            plan_with(&env, &[item(1, 1, 2000, 2100)], &unhealed_busy, &[]),
            &["unscanned catch-up interval 1000..3000"],
        );
    }

    #[test]
    fn rescan_plan_rejects_overlap_and_adjacency_with_open_rows() {
        let env = env();
        let checkpoints = scanned_checkpoints();
        let open = [open_row(1, 1, 3000, 3100), open_row(1, 100, 2500, 2600)];

        for (from, to) in [(3101, 3200), (2900, 2999), (3050, 3060), (2900, 3200)] {
            assert_failed_precondition(
                plan_with(&env, &[item(1, 1, from, to)], &checkpoints, &open),
                &["chain 1 3000..3100"],
            );
        }
        // A gap of one healthy block on either side is enough.
        plan_with(
            &env,
            &[item(1, 1, 3102, 3200), item(1, 1, 2800, 2998)],
            &checkpoints,
            &open,
        )
        .expect("not adjacent to the open row");
        // Another pair's rows do not conflict.
        plan_with(&env, &[item(1, 1, 2500, 2600)], &checkpoints, &open)
            .expect("the open row at 2500..2600 belongs to chain 100");
    }

    #[test]
    fn rescan_plan_overlap_message_has_only_chain_and_bounds() {
        let env = env();
        let checkpoints = scanned_checkpoints();
        let open = [open_row(1, 1, 3000, 3100)];

        let message = assert_failed_precondition(
            plan_with(&env, &[item(1, 1, 3050, 3200)], &checkpoints, &open),
            &["bridge 1, chain 1, 3050..3200", "chain 1 3000..3100"],
        );

        for secret in [
            "secret-token",
            "4242",
            "attempts",
            "2026",
            "15:09",
            "reason",
        ] {
            assert!(
                !message.contains(secret),
                "`{secret}` leaked into: {message}"
            );
        }
    }

    #[test]
    fn rescan_plan_message_indices_follow_the_phase() {
        let env = env();
        let checkpoints = HashMap::from([checkpoint(1, 1, 1000, 999, 100_000)]);

        // Phase 1: only `ranges[0]` (chain 100, below its floor of 2000) is
        // wrong; it is named by its position in the request.
        let message = assert_invalid_argument(
            plan_with(
                &env,
                &[item(1, 100, 1999, 2100), item(1, 1, 5000, 5100)],
                &checkpoints,
                &[],
            ),
            &["ranges[0] (bridge 1, chain 100, 1999..2100)"],
        );
        assert!(!message.contains("ranges[1]"), "{message}");

        // Phase 2: both are valid statically. Normalization puts chain 1
        // first; chain 100 has no checkpoint. The request position is gone, so
        // the message names the normalized range instead.
        let message = assert_failed_precondition(
            plan_with(
                &env,
                &[item(1, 100, 2500, 2600), item(1, 1, 5000, 5100)],
                &checkpoints,
                &[],
            ),
            &["bridge 1, chain 100, 2500..2600: no checkpoint yet"],
        );
        assert!(!message.contains("chain 1, 5000"), "{message}");
    }

    #[test]
    fn rescan_plan_folds_overlapping_and_adjacent_inputs_per_pair() {
        let planned = plan(&[
            item(1, 1, 3000, 3100),
            item(1, 1, 2000, 2100),
            item(1, 1, 2101, 2200),
            item(1, 1, 2150, 2300),
            item(1, 100, 2101, 2200),
            item(1, 100, 2201, 2300),
        ])
        .unwrap();

        assert_eq!(
            planned.scheduled,
            vec![
                (1, 1, range(2000, 2300)),
                (1, 1, range(3000, 3100)),
                (1, 100, range(2101, 2300)),
            ]
        );
        assert_eq!(planned.requested_blocks, 301 + 101 + 200);
    }

    #[test]
    fn rescan_plan_returns_all_violations() {
        // Phase 1: bounds, pair, floor, and the total, in one message.
        let message = assert_invalid_argument(
            plan(&[
                item(1, 1, 2000, 1500),
                item(1, 7, 2000, 2100),
                item(1, 1, 500, 600),
                item(1, 100, 2000, 2_002_000),
            ]),
            &[
                "ranges[0] (bridge 1, chain 1, 2000..1500): from_block must not exceed to_block",
                "ranges[1] (bridge 1, chain 7, 2000..2100): pair is not a configured, enabled indexing target",
                "ranges[2] (bridge 1, chain 1, 500..600): from_block is below the scan floor 1000",
                "too many blocks after merging: 2000001 > 2000000",
            ],
        );
        assert_eq!(message.matches("; ").count(), 3, "{message}");

        // Phase 2: one message for the missing replay profile, the head and the
        // open rows, over both ranges.
        let env = env();
        let checkpoints = scanned_checkpoints();
        let open = [open_row(1, 1, 3000, 3100)];
        let message = assert_failed_precondition(
            plan_with(
                &env,
                &[
                    item(1, 1, 3050, 3200),
                    item(1, 1, 99_000, 100_500),
                    item(2, 1, 2000, 2100),
                ],
                &checkpoints,
                &open,
            ),
            &[
                "bridge 1, chain 1, 3050..3200: overlaps or touches open failed range(s): chain 1 3000..3100",
                "bridge 1, chain 1, 99000..100500: above the last scanned block 99999",
                "bridge 2, chain 1, 2000..2100: failed-range replay is unavailable",
            ],
        );
        assert_eq!(message.matches("; ").count(), 2, "{message}");
    }

    #[test]
    fn rescan_plan_orders_by_bridge_chain_from() {
        let mut env = env();
        env.profiles.insert(4, env.profiles[&1].clone());
        let checkpoints = scanned_checkpoints();

        let planned = plan_with(
            &env,
            &[
                item(4, 1, 5000, 5100),
                item(1, 100, 4000, 4100),
                item(1, 1, 6000, 6100),
                item(1, 100, 3000, 3100),
                item(1, 1, 2000, 2100),
            ],
            &checkpoints,
            &[],
        )
        .unwrap();

        assert_eq!(
            planned.scheduled,
            vec![
                (1, 1, range(2000, 2100)),
                (1, 1, range(6000, 6100)),
                (1, 100, range(3000, 3100)),
                (1, 100, range(4000, 4100)),
                (4, 1, range(5000, 5100)),
            ]
        );
        let bridges: Vec<_> = planned
            .estimates
            .iter()
            .map(|(bridge, _)| *bridge)
            .collect();
        assert_eq!(bridges, vec![1, 4]);
    }

    #[test]
    fn rescan_plan_estimate_for_two_million_avalanche_blocks() {
        let planned = plan_with(
            &env(),
            &[item(1, 1, 5000, 2_004_999)],
            &checkpoints_scanned_to(10_000_000),
            &[],
        )
        .unwrap();
        // 2 000 000 blocks / batch 1000 = 2000 chunks; 8 per tick = 250 ticks;
        // 30 s backoff + 250 * 60 s.
        assert_eq!(planned.estimates, vec![(1, 15_030)]);

        // Chunks are counted per range, not over the bridge's total blocks:
        // three 1500-block ranges need 2 + 2 + 2 chunks, not ceil(4500 / 1000).
        let planned = plan(&[
            item(1, 1, 2000, 3499),
            item(1, 1, 4000, 5499),
            item(1, 100, 6000, 7499),
        ])
        .unwrap();
        assert_eq!(planned.estimates, vec![(1, 30 + 60)]);
    }

    #[test]
    fn rescan_plan_estimate_survives_zero_divisors() {
        let zeroed = |scan_interval, max_chunks_per_pass| ReplayProfile {
            batch_size: 0,
            failure_retry: FailureRetrySettings {
                scan_interval,
                max_chunks_per_pass,
                ..retry_settings(true)
            },
        };

        // batch_size 0 and max_chunks_per_pass 0 count as 1: 100 blocks are 100
        // chunks and 100 ticks.
        assert_eq!(
            estimated_drain_seconds(&zeroed(Duration::from_secs(60), 0), 100),
            30 + 100 * 60
        );
        assert_eq!(
            estimated_drain_seconds(&zeroed(Duration::ZERO, 0), 100),
            30,
            "a zero scan interval is only the backoff"
        );
        assert_eq!(
            estimated_drain_seconds(&zeroed(Duration::from_secs(u64::MAX), 1), u64::MAX),
            u64::MAX,
            "saturates instead of overflowing"
        );

        // Through the whole plan: a profile that was never validated.
        let mut env = env();
        env.profiles.insert(1, zeroed(Duration::from_secs(60), 0));
        let planned =
            plan_with(&env, &[item(1, 1, 2000, 2099)], &scanned_checkpoints(), &[]).unwrap();
        assert_eq!(planned.estimates, vec![(1, 30 + 100 * 60)]);
    }

    fn bridge_config(
        bridge_id: i32,
        bridge_type: BridgeType,
        indexer_type: IndexerType,
        enabled: bool,
    ) -> BridgeConfig {
        BridgeConfig {
            bridge_id,
            name: format!("bridge-{bridge_id}"),
            bridge_type,
            indexer_type,
            enabled,
            api_url: None,
            ui_url: None,
            docs_url: None,
            process_unknown_chains: false,
            home_chain_id: None,
            reconstruct_incoming_ictt_transfers: true,
            contracts: vec![],
        }
    }

    #[test]
    fn rescan_plan_profiles_follow_bridge_and_indexer_type() {
        let mut settings = Settings::default("postgres://unused".to_string());
        settings.avalanche_indexer.batch_size = 11;
        settings.avalanche_indexer.failure_retry.max_chunks_per_pass = 21;
        settings.amb_indexer.batch_size = 12;
        settings.amb_indexer.failure_retry.max_chunks_per_pass = 22;
        settings.xdai_indexer.batch_size = 13;
        settings.xdai_indexer.failure_retry.enabled = false;

        let bridges = [
            bridge_config(1, BridgeType::AvalancheNative, IndexerType::IcmIctt, true),
            bridge_config(2, BridgeType::Amb, IndexerType::AMB, true),
            bridge_config(3, BridgeType::Xdai, IndexerType::XDai, true),
            // The type pair decides, not either half alone.
            bridge_config(4, BridgeType::AvalancheNative, IndexerType::AMB, true),
            bridge_config(5, BridgeType::Amb, IndexerType::Unknown, true),
            bridge_config(6, BridgeType::Xdai, IndexerType::IcmIctt, true),
            // A disabled bridge has no indexer.
            bridge_config(7, BridgeType::Amb, IndexerType::AMB, false),
        ];

        let profiles = build_replay_profiles(&bridges, &settings);

        assert_eq!(profiles.keys().copied().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(
            profiles[&1],
            ReplayProfile {
                batch_size: 11,
                failure_retry: settings.avalanche_indexer.failure_retry.clone(),
            }
        );
        assert_eq!(profiles[&1].failure_retry.max_chunks_per_pass, 21);
        assert_eq!(profiles[&2].batch_size, 12);
        assert_eq!(profiles[&2].failure_retry.max_chunks_per_pass, 22);
        assert_eq!(profiles[&3].batch_size, 13);
        assert!(
            !profiles[&3].failure_retry.enabled,
            "the profile carries the switch; the plan rejects on it"
        );
    }

    #[test]
    fn rescan_plan_truncates_ledger_reason_at_a_char_boundary() {
        assert_eq!(
            ledger_reason("tester", "TICKET-9"),
            "write-api: tester: TICKET-9"
        );

        // The prefix is 19 bytes, so the 500-byte cut lands in the middle of
        // the 2-byte 'é' that starts at byte 499.
        let truncated = ledger_reason("tester", &"é".repeat(400));
        assert_eq!(truncated.len(), 499);
        assert!(truncated.starts_with("write-api: tester: é"));
        assert!(truncated.ends_with('é'));

        let exactly_at_limit = ledger_reason("tester", &"a".repeat(500 - 19));
        assert_eq!(exactly_at_limit.len(), RESCAN_LEDGER_REASON_MAX_BYTES);
    }
}
