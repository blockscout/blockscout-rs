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
    use chrono::{Duration as ChronoDuration, NaiveDate};

    use super::*;

    fn time() -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    }
    fn row(from: u64, to: u64, attempts: u32) -> (i64, FailedInterval) {
        (
            1,
            FailedInterval {
                range: BlockRange { from, to },
                attempts,
                reason: None,
                first_failed_at: time(),
                last_attempt_at: time(),
            },
        )
    }
    fn scheduler(width: u64) -> RetryScheduler {
        RetryScheduler::new(width, 3, Duration::from_secs(1), Duration::from_secs(64))
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
}
