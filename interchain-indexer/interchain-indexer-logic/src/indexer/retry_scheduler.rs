// SPDX-License-Identifier: LicenseRef-Blockscout

//! Pure, in-memory scheduling for durable failed-range coverage.
//!
//! The database ledger is the authority. This module only retains enough
//! state to continue a bounded sweep fairly and to learn a narrower request
//! width after wholly unsuccessful sweeps.

use std::{collections::HashSet, time::Duration};

use chrono::NaiveDateTime;

use super::failure_ledger::{
    BlockRange, FailedInterval, interval::overlaps, policy::next_attempt_at,
};

pub(crate) struct RetryScheduler {
    sessions: Vec<RetrySession>,
    next_id: u64,
    last_served_id: Option<u64>,
    batch_size: u64,
    split_after_attempts: u32,
    backoff_base: Duration,
    backoff_cap: Duration,
    finished_this_tick: HashSet<u64>,
}

#[derive(Clone)]
struct RetrySession {
    id: u64,
    chain_id: i64,
    coverage: BlockRange,
    width: u64,
    failed_sweeps: u32,
    narrowing_started: bool,
    next_due_at: Option<NaiveDateTime>,
    active: Option<ActiveSweep>,
}

#[derive(Clone, Copy)]
struct ActiveSweep {
    next_block: u64,
    fixed_end: u64,
    resolved_any: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ScheduledRetryChunk {
    pub session_id: u64,
    pub chain_id: i64,
    pub range: BlockRange,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RetryChunkOutcome {
    Resolved,
    NotResolved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SweepCompletion {
    pub session_id: u64,
    pub chain_id: i64,
    pub old_width: u64,
    pub new_width: u64,
    pub resolved_any: bool,
    pub failed_sweeps: u32,
    pub narrowing_started: bool,
    pub next_due_at: Option<NaiveDateTime>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct RetryTickStats {
    pub open_sessions: usize,
    pub ready_sessions: usize,
}

impl RetryScheduler {
    pub(crate) fn new(
        batch_size: u64,
        split_after_attempts: u32,
        backoff_base: Duration,
        backoff_cap: Duration,
    ) -> Self {
        Self {
            sessions: vec![],
            next_id: 0,
            last_served_id: None,
            batch_size: batch_size.max(1),
            split_after_attempts: split_after_attempts.max(1),
            backoff_base,
            backoff_cap,
            finished_this_tick: HashSet::new(),
        }
    }

    /// Reconciles a complete durable snapshot. A failed `open` must not call
    /// this method, so existing optimization state remains intact.
    pub(crate) fn begin_tick(
        &mut self,
        rows: &[(i64, FailedInterval)],
        decision_time: NaiveDateTime,
    ) -> RetryTickStats {
        self.finished_this_tick.clear();
        self.reconcile(rows, decision_time);
        RetryTickStats {
            open_sessions: self.sessions.len(),
            ready_sessions: self
                .sessions
                .iter()
                .filter(|session| self.ready(session, decision_time))
                .count(),
        }
    }

    pub(crate) fn next_chunk(
        &mut self,
        decision_time: NaiveDateTime,
    ) -> Option<ScheduledRetryChunk> {
        let mut ready: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| self.ready(session, decision_time))
            .map(|(index, _)| index)
            .collect();
        ready.sort_by_key(|index| self.sessions[*index].id);
        let index = match self.last_served_id {
            Some(last) => ready
                .iter()
                .copied()
                .find(|index| self.sessions[*index].id > last)
                .or_else(|| ready.first().copied()),
            None => ready.first().copied(),
        }?;

        let session = &mut self.sessions[index];
        let active = session.active.get_or_insert(ActiveSweep {
            next_block: session.coverage.from,
            fixed_end: session.coverage.to,
            resolved_any: false,
        });
        let range = BlockRange {
            from: active.next_block,
            to: active
                .next_block
                .saturating_add(session.width.saturating_sub(1))
                .min(active.fixed_end),
        };
        self.last_served_id = Some(session.id);
        Some(ScheduledRetryChunk {
            session_id: session.id,
            chain_id: session.chain_id,
            range,
        })
    }

    pub(crate) fn report_outcome(
        &mut self,
        chunk: ScheduledRetryChunk,
        outcome: RetryChunkOutcome,
        completion_time: NaiveDateTime,
    ) -> Option<SweepCompletion> {
        let session = self
            .sessions
            .iter_mut()
            .find(|session| session.id == chunk.session_id && session.chain_id == chunk.chain_id)?;
        let active = session.active.as_mut()?;
        debug_assert_eq!(active.next_block, chunk.range.from);

        if outcome == RetryChunkOutcome::Resolved {
            active.resolved_any = true;
            session.width = session.width.min(chunk.range.width());
        }
        if chunk.range.to < active.fixed_end {
            active.next_block = chunk.range.to.saturating_add(1);
            return None;
        }
        Self::finish_sweep(
            session,
            self.split_after_attempts,
            self.backoff_base,
            self.backoff_cap,
            completion_time,
            &mut self.finished_this_tick,
        )
    }

    fn ready(&self, session: &RetrySession, decision_time: NaiveDateTime) -> bool {
        !self.finished_this_tick.contains(&session.id)
            && (session.active.is_some()
                || session
                    .next_due_at
                    .is_none_or(|next_due_at| next_due_at <= decision_time))
    }

    fn reconcile(&mut self, rows: &[(i64, FailedInterval)], decision_time: NaiveDateTime) {
        let mut rows = rows.to_vec();
        rows.sort_by_key(|(chain_id, interval)| (*chain_id, interval.range.from));
        let mut old = std::mem::take(&mut self.sessions);
        old.sort_by_key(|session| (session.chain_id, session.coverage.from));
        let mut first_candidate = 0;
        let mut next = Vec::with_capacity(rows.len());
        let mut used_ids = HashSet::with_capacity(rows.len());

        for (chain_id, row) in rows {
            while first_candidate < old.len()
                && (old[first_candidate].chain_id < chain_id
                    || (old[first_candidate].chain_id == chain_id
                        && old[first_candidate].coverage.to < row.range.from))
            {
                first_candidate += 1;
            }
            let mut parents: Vec<&RetrySession> = Vec::new();
            let mut cursor = first_candidate;
            while cursor < old.len()
                && old[cursor].chain_id == chain_id
                && old[cursor].coverage.from <= row.range.to
            {
                if overlaps(old[cursor].coverage, row.range) {
                    parents.push(&old[cursor]);
                }
                cursor += 1;
            }
            let mut session = if parents.is_empty() {
                RetrySession {
                    id: self.allocate_id(&used_ids),
                    chain_id,
                    coverage: row.range,
                    width: self.batch_size.min(row.range.width()),
                    failed_sweeps: row.attempts,
                    narrowing_started: row.attempts >= self.split_after_attempts,
                    next_due_at: next_attempt_at(
                        row.last_attempt_at,
                        row.attempts,
                        self.backoff_base,
                        self.backoff_cap,
                    ),
                    active: None,
                }
            } else {
                let primary = parents
                    .iter()
                    .copied()
                    .filter(|parent| parent.active.is_some())
                    .min_by_key(|parent| parent.id)
                    .or_else(|| parents.iter().copied().min_by_key(|parent| parent.id))
                    .expect("parents is non-empty");
                let width = parents
                    .iter()
                    .map(|parent| parent.width)
                    .fold(row.range.width(), u64::min)
                    .min(row.range.width());
                let next_due_at = parents
                    .iter()
                    .map(|parent| parent.next_due_at)
                    .reduce(earliest_due)
                    .expect("parents is non-empty");
                let mut active = primary.active;
                if let Some(active_sweep) = &mut active {
                    active_sweep.next_block = active_sweep.next_block.max(row.range.from);
                    active_sweep.fixed_end = active_sweep.fixed_end.min(row.range.to);
                }
                RetrySession {
                    id: primary.id,
                    chain_id,
                    coverage: row.range,
                    width,
                    failed_sweeps: parents
                        .iter()
                        .map(|parent| parent.failed_sweeps)
                        .max()
                        .unwrap_or_default(),
                    narrowing_started: parents.iter().any(|parent| parent.narrowing_started),
                    next_due_at,
                    active,
                }
            };

            if !used_ids.insert(session.id) {
                session.id = self.allocate_id(&used_ids);
                used_ids.insert(session.id);
            }
            if let Some(active) = session.active
                && active.next_block > active.fixed_end
            {
                // Coverage which was not yet visited disappeared from the
                // durable snapshot, so this is confirmed progress. Do not
                // immediately begin another sweep of the changed row.
                session.active = Some(ActiveSweep {
                    resolved_any: true,
                    ..active
                });
                let _ = Self::finish_sweep(
                    &mut session,
                    self.split_after_attempts,
                    self.backoff_base,
                    self.backoff_cap,
                    decision_time,
                    &mut self.finished_this_tick,
                );
            }
            next.push(session);
        }
        self.sessions = next;
    }

    fn allocate_id(&mut self, used: &HashSet<u64>) -> u64 {
        loop {
            let id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
            if !used.contains(&id) {
                return id;
            }
        }
    }

    fn finish_sweep(
        session: &mut RetrySession,
        split_after_attempts: u32,
        backoff_base: Duration,
        backoff_cap: Duration,
        completion_time: NaiveDateTime,
        finished_this_tick: &mut HashSet<u64>,
    ) -> Option<SweepCompletion> {
        let active = session.active.take()?;
        let old_width = session.width;
        if active.resolved_any {
            session.failed_sweeps = 0;
            session.next_due_at = next_attempt_at(completion_time, 0, backoff_base, backoff_cap);
        } else {
            session.failed_sweeps = session.failed_sweeps.saturating_add(1);
            if session.failed_sweeps >= split_after_attempts {
                session.narrowing_started = true;
            }
            if session.narrowing_started {
                session.width = (session.width / 2 + session.width % 2).max(1);
            }
            session.next_due_at = next_attempt_at(
                completion_time,
                session.failed_sweeps,
                backoff_base,
                backoff_cap,
            );
        }
        finished_this_tick.insert(session.id);
        Some(SweepCompletion {
            session_id: session.id,
            chain_id: session.chain_id,
            old_width,
            new_width: session.width,
            resolved_any: active.resolved_any,
            failed_sweeps: session.failed_sweeps,
            narrowing_started: session.narrowing_started,
            next_due_at: session.next_due_at,
        })
    }
}

fn earliest_due(
    left: Option<NaiveDateTime>,
    right: Option<NaiveDateTime>,
) -> Option<NaiveDateTime> {
    match (left, right) {
        (None, _) | (_, None) => None,
        (Some(left), Some(right)) => Some(left.min(right)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::{Duration as ChronoDuration, NaiveDate};

    use super::*;

    fn time() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }
    fn row(from: u64, to: u64, attempts: u32) -> (i64, FailedInterval) {
        row_on(1, from, to, attempts, time())
    }
    fn row_on(
        chain_id: i64,
        from: u64,
        to: u64,
        attempts: u32,
        last_attempt_at: NaiveDateTime,
    ) -> (i64, FailedInterval) {
        (
            chain_id,
            FailedInterval {
                range: BlockRange { from, to },
                attempts,
                reason: None,
                first_failed_at: last_attempt_at,
                last_attempt_at,
            },
        )
    }
    fn scheduler(width: u64) -> RetryScheduler {
        RetryScheduler::new(width, 3, Duration::from_secs(1), Duration::from_secs(64))
    }

    fn fail_sweep(scheduler: &mut RetryScheduler, decision_time: NaiveDateTime) -> SweepCompletion {
        loop {
            let chunk = scheduler.next_chunk(decision_time).unwrap();
            if let Some(completion) =
                scheduler.report_outcome(chunk, RetryChunkOutcome::NotResolved, decision_time)
            {
                return completion;
            }
        }
    }

    #[test]
    fn bootstrap_uses_row_attempts_for_due_and_initial_state() {
        let mut scheduler = scheduler(8);
        let decision_time = time() + ChronoDuration::seconds(1);

        let stats = scheduler.begin_tick(&[row(10, 12, 1)], decision_time);

        assert_eq!(stats.open_sessions, 1);
        assert_eq!(stats.ready_sessions, 1);
        let session = &scheduler.sessions[0];
        assert_eq!(session.coverage, BlockRange { from: 10, to: 12 });
        assert_eq!(session.width, 3);
        assert_eq!(session.failed_sweeps, 1);
        assert!(!session.narrowing_started);
        assert_eq!(session.next_due_at, Some(decision_time));
    }

    #[test]
    fn large_bootstrap_attempt_count_starts_narrowing_without_retroactive_halving() {
        let mut scheduler = scheduler(8);
        let now = time() + ChronoDuration::seconds(64);

        scheduler.begin_tick(&[row(0, 31, u32::MAX)], now);

        let session = &scheduler.sessions[0];
        assert_eq!(session.width, 8);
        assert_eq!(session.failed_sweeps, u32::MAX);
        assert!(session.narrowing_started);
        let completion = fail_sweep(&mut scheduler, now);
        assert_eq!(completion.failed_sweeps, u32::MAX);
        assert_eq!(completion.new_width, 4);
    }

    #[test]
    fn threshold_halves_once_after_a_wholly_failed_sweep() {
        let mut scheduler = scheduler(8);
        let first_time = time() + ChronoDuration::seconds(1);
        scheduler.begin_tick(&[row(0, 7, 1)], first_time);
        let chunk = scheduler.next_chunk(first_time).unwrap();
        let completion = scheduler
            .report_outcome(chunk, RetryChunkOutcome::NotResolved, first_time)
            .unwrap();
        assert_eq!(completion.new_width, 8);
        let second_time = time() + ChronoDuration::seconds(3);
        scheduler.begin_tick(&[row(0, 7, 1)], second_time);
        let chunk = scheduler.next_chunk(second_time).unwrap();
        let completion = scheduler
            .report_outcome(chunk, RetryChunkOutcome::NotResolved, second_time)
            .unwrap();
        assert_eq!(completion.new_width, 4);
    }

    #[test]
    fn a_multi_chunk_sweep_halves_only_once_and_rounds_odd_width_up() {
        let mut scheduler = scheduler(5);
        let now = time() + ChronoDuration::seconds(2);
        scheduler.begin_tick(&[row(0, 11, 2)], now);

        let first = scheduler.next_chunk(now).unwrap();
        assert_eq!(first.range, BlockRange { from: 0, to: 4 });
        assert!(
            scheduler
                .report_outcome(first, RetryChunkOutcome::NotResolved, now)
                .is_none()
        );
        assert_eq!(scheduler.sessions[0].width, 5);

        let completion = fail_sweep(&mut scheduler, now);
        assert_eq!(completion.old_width, 5);
        assert_eq!(completion.new_width, 3);
    }

    #[test]
    fn singleton_width_never_gives_up() {
        let mut scheduler = scheduler(1);
        let row = row(7, 7, 3);

        for pass in 1..=6 {
            let now = time() + ChronoDuration::seconds(pass * 100);
            scheduler.begin_tick(std::slice::from_ref(&row), now);
            let completion = fail_sweep(&mut scheduler, now);
            assert_eq!(completion.new_width, 1);
            assert!(completion.narrowing_started);
        }
    }

    #[test]
    fn active_sweep_crosses_ticks_and_round_robins() {
        let mut scheduler = scheduler(2);
        let now = time() + ChronoDuration::seconds(4);
        scheduler.begin_tick(&[row(0, 3, 1), row(10, 11, 1)], now);
        let first = scheduler.next_chunk(now).unwrap();
        assert_eq!(first.range, BlockRange { from: 0, to: 1 });
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, now);
        let second = scheduler.next_chunk(now).unwrap();
        assert_eq!(second.range, BlockRange { from: 10, to: 11 });
    }

    #[test]
    fn active_sweep_ignores_backoff_and_cannot_restart_after_completion_in_same_tick() {
        let mut scheduler = scheduler(2);
        let now = time() + ChronoDuration::seconds(4);
        let rows = [row(0, 3, 1)];
        scheduler.begin_tick(&rows, now);
        let first = scheduler.next_chunk(now).unwrap();
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, now);

        scheduler.begin_tick(&rows, time());
        let second = scheduler.next_chunk(time()).unwrap();
        assert_eq!(second.range, BlockRange { from: 2, to: 3 });
        assert!(
            scheduler
                .report_outcome(second, RetryChunkOutcome::NotResolved, time())
                .is_some()
        );
        assert!(scheduler.next_chunk(time()).is_none());
    }

    #[test]
    fn round_robin_shares_budgets_across_intervals_and_chains() {
        for budget in [1usize, 2, 8] {
            let mut scheduler = scheduler(2);
            let now = time() + ChronoDuration::seconds(4);
            let rows = [
                row_on(2, 20, 23, 1, time()),
                row_on(1, 0, 3, 1, time()),
                row_on(1, 10, 13, 1, time()),
            ];
            scheduler.begin_tick(&rows, now);

            let mut chunks = Vec::new();
            for _ in 0..budget {
                let Some(chunk) = scheduler.next_chunk(now) else {
                    break;
                };
                chunks.push((chunk.chain_id, chunk.range));
                scheduler.report_outcome(chunk, RetryChunkOutcome::NotResolved, now);
            }

            let expected = [
                (1, BlockRange { from: 0, to: 1 }),
                (1, BlockRange { from: 10, to: 11 }),
                (2, BlockRange { from: 20, to: 21 }),
                (1, BlockRange { from: 2, to: 3 }),
                (1, BlockRange { from: 12, to: 13 }),
                (2, BlockRange { from: 22, to: 23 }),
            ];
            assert_eq!(chunks, expected[..budget.min(expected.len())]);
        }
    }

    #[test]
    fn staggered_due_sessions_become_ready_without_starving_later_ids() {
        let mut scheduler = scheduler(1);
        let first_due = time() + ChronoDuration::seconds(1);
        let second_due = time() + ChronoDuration::seconds(11);
        let rows = [
            row_on(1, 0, 0, 1, time()),
            row_on(2, 0, 0, 1, time() + ChronoDuration::seconds(10)),
        ];

        let stats = scheduler.begin_tick(&rows, first_due);
        assert_eq!(stats.ready_sessions, 1);
        let first = scheduler.next_chunk(first_due).unwrap();
        assert_eq!(first.chain_id, 1);
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, first_due);
        assert!(scheduler.next_chunk(first_due).is_none());

        scheduler.begin_tick(&rows, second_due);
        let next = scheduler.next_chunk(second_due).unwrap();
        assert_eq!(next.chain_id, 2);
    }

    #[test]
    fn changed_db_attempts_and_timestamp_do_not_reschedule_an_existing_session() {
        let mut scheduler = scheduler(8);
        scheduler.begin_tick(&[row(0, 7, 1)], time());
        let original_due = scheduler.sessions[0].next_due_at;

        scheduler.begin_tick(
            &[row_on(1, 0, 7, 99, time() + ChronoDuration::hours(1))],
            time(),
        );

        assert_eq!(scheduler.sessions[0].next_due_at, original_due);
        assert_eq!(scheduler.sessions[0].failed_sweeps, 1);
    }

    #[test]
    fn adjacent_growth_does_not_extend_an_active_sweep() {
        let mut scheduler = scheduler(2);
        let now = time() + ChronoDuration::seconds(1);
        scheduler.begin_tick(&[row(0, 3, 1)], now);
        let first = scheduler.next_chunk(now).unwrap();
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, now);

        scheduler.begin_tick(&[row(0, 9, 2)], now);
        let second = scheduler.next_chunk(now).unwrap();
        assert_eq!(second.range, BlockRange { from: 2, to: 3 });
        assert!(
            scheduler
                .report_outcome(second, RetryChunkOutcome::NotResolved, now)
                .is_some()
        );
    }

    #[test]
    fn split_remainders_inherit_state_with_unique_ids() {
        let mut scheduler = scheduler(4);
        let now = time() + ChronoDuration::seconds(2);
        scheduler.begin_tick(&[row(0, 9, 2)], now);
        let first = scheduler.next_chunk(now).unwrap();
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, now);

        scheduler.begin_tick(&[row(0, 1, 1), row(4, 5, 1), row(7, 9, 1)], now);

        assert_eq!(scheduler.sessions.len(), 3);
        let ids: HashSet<_> = scheduler
            .sessions
            .iter()
            .map(|session| session.id)
            .collect();
        assert_eq!(ids.len(), 3);
        assert!(scheduler.sessions.iter().all(|session| session.width <= 4));
        assert!(
            scheduler
                .sessions
                .iter()
                .any(|session| session.coverage == BlockRange { from: 4, to: 5 })
        );
        assert!(
            scheduler
                .sessions
                .iter()
                .any(|session| session.coverage == BlockRange { from: 7, to: 9 })
        );
    }

    #[test]
    fn prefix_suffix_and_interior_splits_preserve_scheduler_hints() {
        let now = time() + ChronoDuration::seconds(8);
        let cases = [
            vec![row(2, 9, 1)],
            vec![row(0, 7, 1)],
            vec![row(0, 3, 1), row(6, 9, 1)],
        ];

        for rows in cases {
            let mut scheduler = scheduler(8);
            scheduler.begin_tick(&[row(0, 9, 3)], now);
            let parent_id = scheduler.sessions[0].id;
            scheduler.sessions[0].width = 3;
            scheduler.sessions[0].failed_sweeps = 6;
            scheduler.sessions[0].next_due_at = Some(now + ChronoDuration::seconds(20));

            scheduler.begin_tick(&rows, now);

            assert_eq!(scheduler.sessions.len(), rows.len());
            assert_eq!(scheduler.sessions[0].id, parent_id);
            let ids: HashSet<_> = scheduler
                .sessions
                .iter()
                .map(|session| session.id)
                .collect();
            assert_eq!(ids.len(), rows.len());
            assert!(scheduler.sessions.iter().all(|session| {
                session.width == 3
                    && session.failed_sweeps == 6
                    && session.narrowing_started
                    && session.next_due_at == Some(now + ChronoDuration::seconds(20))
            }));
        }
    }

    #[test]
    fn merge_uses_active_primary_and_combines_parent_hints() {
        let mut scheduler = scheduler(8);
        let now = time() + ChronoDuration::seconds(8);
        scheduler.begin_tick(&[row(0, 3, 1), row(6, 9, 1)], now);
        scheduler.sessions[0].width = 4;
        scheduler.sessions[0].failed_sweeps = 2;
        scheduler.sessions[0].next_due_at = Some(now + ChronoDuration::seconds(20));
        scheduler.sessions[1].width = 2;
        scheduler.sessions[1].failed_sweeps = 7;
        scheduler.sessions[1].narrowing_started = true;
        scheduler.sessions[1].next_due_at = Some(now + ChronoDuration::seconds(10));
        scheduler.sessions[1].active = Some(ActiveSweep {
            next_block: 8,
            fixed_end: 9,
            resolved_any: false,
        });
        let active_id = scheduler.sessions[1].id;

        scheduler.begin_tick(&[row(0, 9, 8)], now);

        let merged = &scheduler.sessions[0];
        assert_eq!(merged.id, active_id);
        assert_eq!(merged.width, 2);
        assert_eq!(merged.failed_sweeps, 7);
        assert!(merged.narrowing_started);
        assert_eq!(merged.next_due_at, Some(now + ChronoDuration::seconds(10)));
        let active = merged.active.unwrap();
        assert_eq!(active.next_block, 8);
        assert_eq!(active.fixed_end, 9);
    }

    #[test]
    fn removed_tail_counts_as_progress_and_reappearing_coverage_is_fresh() {
        let mut scheduler = scheduler(4);
        let now = time() + ChronoDuration::seconds(2);
        scheduler.begin_tick(&[row(0, 9, 2)], now);
        let first = scheduler.next_chunk(now).unwrap();
        let old_id = first.session_id;
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, now);

        scheduler.begin_tick(&[row(0, 3, 1)], now);
        assert_eq!(scheduler.sessions[0].failed_sweeps, 0);
        assert!(scheduler.sessions[0].active.is_none());
        assert!(scheduler.next_chunk(now).is_none());

        scheduler.begin_tick(&[], now);
        scheduler.begin_tick(&[row(0, 3, 1)], now);
        assert_ne!(scheduler.sessions[0].id, old_id);
        assert_eq!(scheduler.sessions[0].width, 4);
        assert_eq!(scheduler.sessions[0].failed_sweeps, 1);
    }

    #[test]
    fn numeric_and_chrono_boundaries_are_safe_and_due_fail_open() {
        let mut scheduler = RetryScheduler::new(
            u64::MAX,
            3,
            Duration::from_secs(u64::MAX),
            Duration::from_secs(u64::MAX),
        );
        let max_time = NaiveDateTime::MAX;
        scheduler.begin_tick(
            &[
                row_on(1, 0, 0, u32::MAX, max_time),
                row_on(2, i64::MAX as u64, i64::MAX as u64, u32::MAX, max_time),
            ],
            NaiveDateTime::MIN,
        );

        assert!(
            scheduler
                .sessions
                .iter()
                .all(|session| session.next_due_at.is_none())
        );
        let first = scheduler.next_chunk(NaiveDateTime::MIN).unwrap();
        assert_eq!(first.range.width(), 1);
        scheduler.report_outcome(first, RetryChunkOutcome::NotResolved, NaiveDateTime::MAX);
        let second = scheduler.next_chunk(NaiveDateTime::MIN).unwrap();
        assert_eq!(second.range.width(), 1);
    }

    #[test]
    fn not_resolved_outcome_never_counts_as_progress() {
        let mut scheduler = scheduler(8);
        let now = time() + ChronoDuration::seconds(2);
        scheduler.begin_tick(&[row(0, 7, 2)], now);
        let completion = fail_sweep(&mut scheduler, now);

        assert!(!completion.resolved_any);
        assert_eq!(completion.failed_sweeps, 3);
        assert_eq!(completion.new_width, 4);
    }

    #[test]
    fn progress_uses_actual_short_chunk_width_and_backoff() {
        let mut scheduler = scheduler(8);
        let now = time() + ChronoDuration::seconds(4);
        scheduler.begin_tick(&[row(0, 2, 3)], now);
        let chunk = scheduler.next_chunk(now).unwrap();
        let completion = scheduler
            .report_outcome(chunk, RetryChunkOutcome::Resolved, now)
            .unwrap();
        assert_eq!(completion.new_width, 3);
        assert_eq!(completion.failed_sweeps, 0);
        assert!(completion.narrowing_started);
        assert_eq!(
            completion.next_due_at,
            Some(now + ChronoDuration::seconds(1))
        );
    }

    #[test]
    fn trillion_block_range_is_not_materialized() {
        let mut scheduler = scheduler(1);
        let now = time() + ChronoDuration::seconds(1);
        scheduler.begin_tick(&[row(0, 1_000_000_000_000, 1)], now);
        assert_eq!(scheduler.sessions.len(), 1);
        assert_eq!(
            scheduler.next_chunk(now).unwrap().range,
            BlockRange { from: 0, to: 0 }
        );
    }

    #[test]
    fn exhaustive_small_poison_sets_converge_and_reach_singletons() {
        const BLOCKS: u64 = 5;
        const TICKS: usize = 40;

        for mask in 0u32..(1 << BLOCKS) {
            let poison: HashSet<u64> = (0..BLOCKS)
                .filter(|block| mask & (1 << *block) != 0)
                .collect();
            let mut open: HashSet<u64> = (0..BLOCKS).collect();
            let mut poison_singletons = HashSet::new();
            let mut scheduler = scheduler(BLOCKS);

            for tick in 0..TICKS {
                let now = time() + ChronoDuration::seconds(1000 + tick as i64 * 100);
                let rows: Vec<_> = contiguous_ranges(&open)
                    .into_iter()
                    .map(|range| row(range.from, range.to, 1))
                    .collect();
                scheduler.begin_tick(&rows, now);

                for _ in 0..8 {
                    let Some(chunk) = scheduler.next_chunk(now) else {
                        break;
                    };
                    let contains_poison =
                        (chunk.range.from..=chunk.range.to).any(|block| poison.contains(&block));
                    let outcome = if contains_poison {
                        if chunk.range.width() == 1 {
                            poison_singletons.insert(chunk.range.from);
                        }
                        RetryChunkOutcome::NotResolved
                    } else {
                        for block in chunk.range.from..=chunk.range.to {
                            open.remove(&block);
                        }
                        RetryChunkOutcome::Resolved
                    };
                    scheduler.report_outcome(chunk, outcome, now);
                }

                if open == poison && poison_singletons == poison {
                    break;
                }
            }

            assert_eq!(
                open, poison,
                "ledger oracle mismatch for poison mask {mask:#07b}"
            );
            assert_eq!(
                poison_singletons, poison,
                "not every poison block reached a singleton request for mask {mask:#07b}"
            );
        }
    }

    fn contiguous_ranges(blocks: &HashSet<u64>) -> Vec<BlockRange> {
        let mut blocks: Vec<_> = blocks.iter().copied().collect();
        blocks.sort_unstable();
        let mut ranges = Vec::new();
        let Some(mut from) = blocks.first().copied() else {
            return ranges;
        };
        let mut to = from;
        for block in blocks.into_iter().skip(1) {
            if block == to + 1 {
                to = block;
            } else {
                ranges.push(BlockRange { from, to });
                from = block;
                to = block;
            }
        }
        ranges.push(BlockRange { from, to });
        ranges
    }
}
