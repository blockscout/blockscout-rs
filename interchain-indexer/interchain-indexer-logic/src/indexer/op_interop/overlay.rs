// SPDX-License-Identifier: LicenseRef-Blockscout

//! The optimistic overlay loop (ADR-022): per chain, reads the unsafe window
//! above the confirmed stream, hands it to the indexer as provisional, and
//! runs the confirmation sweep. It is not a scanner of record: it writes no
//! checkpoint and no failure row, and its errors are counted, never retried.

use alloy::rpc::types::Log;

use crate::{
    indexer::metrics::LOG_STREAM_HEAD_BLOCK,
    log_stream::{HeadBlockTag, fetch_logs, is_get_logs_error_silent, resolve_head_block},
};

use super::{
    events::ReadMode,
    indexer::{OpInteropIndexer, RunContext},
    metrics::OP_INTEROP_OVERLAY_ERRORS_TOTAL,
    sweep::{SweepOutcome, sweep},
};

/// What survives between ticks of one chain's overlay. Not persisted: after
/// a restart the first tick starts above the confirmation tag head.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct OverlayState {
    pub(super) next_from: Option<u64>,
}

/// What one tick did, for tests and logs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct TickReport {
    pub(super) swept: Option<SweepOutcome>,
    /// The `eth_getLogs` range read, if any.
    pub(super) read: Option<(u64, u64)>,
    /// Logs passed to `process_batch` after the reach filter.
    pub(super) processed_logs: usize,
}

/// One chain's overlay, forever. Ticks every `pull_interval_ms`
/// (`MissedTickBehavior::Delay`). Never returns; dropped with the indexer's
/// run future.
pub(super) async fn run_chain(ctx: RunContext, chain_id: i64) {
    let mut interval = tokio::time::interval(ctx.settings.pull_interval_ms);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut state = OverlayState::default();
    loop {
        interval.tick().await;
        tick(&ctx, chain_id, &mut state).await;
    }
}

fn count_error(ctx: &RunContext, chain_id: i64, stage: &str) {
    OP_INTEROP_OVERLAY_ERRORS_TOTAL
        .with_label_values(&[&ctx.bridge_id.to_string(), &chain_id.to_string(), stage])
        .inc();
}

/// One tick (ADR-022). Never returns an error: every failure is logged with
/// bridge and chain, counted in `OP_INTEROP_OVERLAY_ERRORS_TOTAL` by `stage`,
/// and ends the tick.
pub(super) async fn tick(ctx: &RunContext, chain_id: i64, state: &mut OverlayState) -> TickReport {
    let bridge_id = ctx.bridge_id;
    let mut report = TickReport::default();
    let (Some(chain), Some(progress)) = (
        ctx.chains.iter().find(|chain| chain.chain_id == chain_id),
        ctx.progress.get(&chain_id),
    ) else {
        tracing::error!(
            bridge_id,
            chain_id,
            "overlay tick for an unconfigured chain"
        );
        return report;
    };
    let provider = &chain.provider;

    // 1. Sweep, always: provisional rows of an earlier run must resolve even
    // with the overlay off. The read below does not depend on it.
    let frontier = progress
        .sweep_frontier
        .load(std::sync::atomic::Ordering::SeqCst);
    match sweep(ctx, chain_id, frontier).await {
        Ok(outcome) => report.swept = Some(outcome),
        Err(err) => {
            tracing::warn!(err = ?err, bridge_id, chain_id, frontier, "overlay sweep failed");
            count_error(ctx, chain_id, "sweep");
        }
    }

    // 2.
    if !ctx.settings.optimistic_overlay {
        return report;
    }

    // 3. Latest.
    let head = match resolve_head_block(provider, HeadBlockTag::Latest).await {
        Ok(Some(head)) => {
            LOG_STREAM_HEAD_BLOCK
                .with_label_values(&[&bridge_id.to_string(), &chain_id.to_string(), "latest"])
                .set(head as f64);
            head
        }
        Ok(None) => {
            tracing::debug!(bridge_id, chain_id, "the latest block is not available yet");
            count_error(ctx, chain_id, "head");
            return report;
        }
        Err(err) => {
            tracing::warn!(
                err = ?err,
                bridge_id,
                chain_id,
                "overlay could not resolve the latest block"
            );
            count_error(ctx, chain_id, "head");
            return report;
        }
    };

    // 4. First tick after a start: start above the confirmation tag head.
    let next_from = match state.next_from {
        Some(next_from) => next_from,
        None => {
            let tag = ctx.settings.confirmation_block_tag;
            match resolve_head_block(provider, tag.into()).await {
                Ok(Some(tag_head)) => {
                    let next_from = tag_head.saturating_add(1);
                    state.next_from = Some(next_from);
                    tracing::info!(
                        bridge_id,
                        chain_id,
                        next_from,
                        "overlay starts above the confirmation tag head"
                    );
                    next_from
                }
                Ok(None) => {
                    tracing::debug!(
                        bridge_id,
                        chain_id,
                        confirmation_block_tag = tag.as_str(),
                        "the confirmation tag head is not available yet"
                    );
                    count_error(ctx, chain_id, "head");
                    return report;
                }
                Err(err) => {
                    tracing::warn!(
                        err = ?err,
                        bridge_id,
                        chain_id,
                        confirmation_block_tag = tag.as_str(),
                        "overlay could not resolve the confirmation tag head"
                    );
                    count_error(ctx, chain_id, "head");
                    return report;
                }
            }
        }
    };

    // 5. Range.
    let reach = *progress.confirmed_reach.lock().await;
    let from = next_from.max(reach.saturating_add(1));
    if from > head {
        tracing::debug!(
            bridge_id,
            chain_id,
            from,
            head,
            "overlay has nothing to read"
        );
        return report;
    }
    let to = head.min(from.saturating_add(ctx.settings.batch_size.saturating_sub(1)));

    // 6. Fetch, no lock held.
    let filter = match ctx.registry.filter_for_chain(chain_id) {
        Ok(filter) => filter,
        Err(err) => {
            tracing::warn!(err = ?err, bridge_id, chain_id, "overlay has no log filter");
            count_error(ctx, chain_id, "fetch");
            return report;
        }
    };
    let logs: Vec<Log> = match fetch_logs(provider.clone(), &filter, from, to).await {
        Ok(logs) => logs,
        Err(err) => {
            if is_get_logs_error_silent(&err) {
                tracing::debug!(
                    err = ?err, bridge_id, chain_id, from, to,
                    "overlay logs are not available at the reported latest block yet"
                );
            } else {
                tracing::warn!(err = ?err, bridge_id, chain_id, from, to, "overlay failed to fetch logs");
            }
            count_error(ctx, chain_id, "fetch");
            return report;
        }
    };
    report.read = Some((from, to));
    tracing::debug!(
        bridge_id,
        chain_id,
        from,
        to,
        count = logs.len(),
        "overlay read a range"
    );

    // 7. Process, under the reach lock: the confirmed stream raises the reach
    // before it writes, so a log at or below it is its own.
    {
        let reach = progress.confirmed_reach.lock().await;
        let logs: Vec<Log> = logs
            .into_iter()
            .filter(|log| log.block_number.is_some_and(|number| number > *reach))
            .collect();
        report.processed_logs = logs.len();
        if !logs.is_empty()
            && let Err(err) =
                OpInteropIndexer::process_batch(ctx, chain_id, &logs, ReadMode::Overlay).await
        {
            tracing::warn!(
                err = ?err.error,
                bridge_id,
                chain_id,
                from,
                to,
                "overlay failed to process logs"
            );
            count_error(ctx, chain_id, "process");
        }
    }

    // 8.
    state.next_from = Some(to.saturating_add(1));
    report
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use interchain_indexer_entity::op_interop_events;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;
    use crate::{
        InterchainDatabase,
        indexer::{
            op_interop::{
                indexer::tests::{
                    BRIDGE_ID, DESTINATION, SOURCE, feed_overlay, ledger, ledger_lock_waiters,
                    lock_ledger, messages, new_buffer, run_context, send_batch, setup,
                    wait_for_ledger_lock_waiters,
                },
                test_fixtures::relay_logs,
            },
            range_driver::RangeProcessor,
        },
        log_stream::{
            LogBatch, ScanDirection,
            test_support::{HeadMockService, LogsGate, ScriptedLogs, TaggedHead},
        },
        message_buffer::MessageBuffer,
    };

    /// Both chains on `mock`, the overlay on and `batch_size` 100. The
    /// `DESTINATION` reach and frontier are `895_600`.
    async fn overlay_context(
        db: &InterchainDatabase,
        buffer: std::sync::Arc<MessageBuffer<super::super::types::Message>>,
        mock: &HeadMockService,
        overlay: bool,
    ) -> RunContext {
        let mut ctx = run_context(db, buffer);
        for chain in &mut ctx.chains {
            chain.provider = mock.provider();
        }
        ctx.settings.optimistic_overlay = overlay;
        ctx.settings.batch_size = 100;
        ctx.progress[&DESTINATION].reset(895_600).await;
        ctx
    }

    fn count(mock: &HeadMockService, method: &str) -> usize {
        mock.calls().iter().filter(|(m, _)| m == method).count()
    }

    fn empty() -> ScriptedLogs {
        ScriptedLogs::Logs(vec![])
    }

    async fn all_rows_have_state(db: &InterchainDatabase, state: &str) {
        let rows = ledger(db).await;
        assert!(!rows.is_empty(), "the ledger has rows");
        for row in rows {
            assert_eq!(row.state, state, "{row:?}");
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_first_tick_starts_above_the_tag_head() {
        let (_guard, db) = setup("op_overlay_loop_first_tick").await;
        let mock = HeadMockService::new(895_800, vec![TaggedHead::Block(895_650)])
            .with_logs(vec![ScriptedLogs::Logs(relay_logs())]);
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = overlay_context(&db, buffer.clone(), &mock, true).await;
        let mut state = OverlayState::default();

        let report = tick(&ctx, DESTINATION, &mut state).await;

        let tag_calls: Vec<_> = mock
            .calls()
            .into_iter()
            .filter(|(method, _)| method == "eth_getBlockByNumber")
            .collect();
        assert_eq!(tag_calls.len(), 1);
        assert_eq!(tag_calls[0].1, json!(["finalized", false]));
        assert_eq!(report.read, Some((895_651, 895_750)));
        all_rows_have_state(&db, "provisional").await;
        assert!(
            db.open_indexer_failures(&[(BRIDGE_ID, DESTINATION)])
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(state.next_from, Some(895_751));
        buffer.run().await.unwrap();
        assert!(
            messages(&db).await.is_empty(),
            "the destination leg waits in the buffer for its source"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_reads_nothing_while_the_tag_head_is_unavailable() {
        let (_guard, db) = setup("op_overlay_loop_tag_unavailable").await;
        let mock = HeadMockService::new(
            895_800,
            vec![
                TaggedHead::Null,
                TaggedHead::Error,
                TaggedHead::Block(895_650),
            ],
        )
        .with_logs(vec![empty()]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState::default();

        for _ in 0..2 {
            let report = tick(&ctx, DESTINATION, &mut state).await;
            assert_eq!(report.read, None);
            assert_eq!(state.next_from, None);
        }
        assert_eq!(count(&mock, "eth_getLogs"), 0);

        let report = tick(&ctx, DESTINATION, &mut state).await;
        assert_eq!(report.read, Some((895_651, 895_750)));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_later_ticks_follow_next_from_and_the_reach() {
        let (_guard, db) = setup("op_overlay_loop_later_ticks").await;
        let mock = HeadMockService::new(895_800, vec![TaggedHead::Block(895_650)])
            .with_logs(vec![empty(), empty()]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState::default();

        tick(&ctx, DESTINATION, &mut state).await;
        assert_eq!(state.next_from, Some(895_751));

        mock.set_latest(895_900);
        ctx.progress[&DESTINATION].raise_reach(895_760).await;
        let report = tick(&ctx, DESTINATION, &mut state).await;

        assert_eq!(report.read, Some((895_761, 895_860)));
        assert_eq!(
            count(&mock, "eth_getBlockByNumber"),
            1,
            "the tag head is resolved once per start"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_drops_logs_below_a_reach_raised_during_its_fetch() {
        let (_guard, db) = setup("op_overlay_loop_reach_during_fetch").await;
        let gate = LogsGate::default();
        let mock = HeadMockService::new(895_800, vec![])
            .with_logs(vec![ScriptedLogs::Gated(relay_logs(), gate.clone())]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState {
            next_from: Some(895_690),
        };

        let task_ctx = ctx.clone();
        let task = tokio::spawn(async move {
            let report = tick(&task_ctx, DESTINATION, &mut state).await;
            (report, state)
        });
        gate.entered().await;

        // The confirmed stream raises the reach while the answer is held.
        tokio::time::timeout(
            Duration::from_secs(5),
            ctx.process(
                DESTINATION,
                &LogBatch {
                    from_block: 895_691,
                    to_block: 895_701,
                    direction: ScanDirection::Realtime,
                    logs: vec![],
                },
            ),
        )
        .await
        .expect("the overlay holds the reach lock across eth_getLogs")
        .unwrap_or_else(|err| panic!("{:#}", err.error));

        gate.release();
        let (report, state) = task.await.unwrap();

        assert_eq!(report.read, Some((895_690, 895_789)));
        assert_eq!(report.processed_logs, 0);
        assert!(
            ledger(&db)
                .await
                .iter()
                .all(|row: &op_interop_events::Model| row.block_number != 895_701)
        );
        assert_eq!(state.next_from, Some(895_790));
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_holds_the_reach_lock_while_it_writes() {
        let (_guard, db) = setup("op_overlay_loop_holds_reach_lock").await;
        let mock =
            HeadMockService::new(895_800, vec![]).with_logs(vec![ScriptedLogs::Logs(relay_logs())]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState {
            next_from: Some(895_690),
        };

        let locker = lock_ledger(&db).await;
        let task_ctx = ctx.clone();
        let task = tokio::spawn(async move { tick(&task_ctx, DESTINATION, &mut state).await });
        wait_for_ledger_lock_waiters(&db, 1).await;

        assert!(
            ctx.progress[&DESTINATION]
                .confirmed_reach
                .try_lock()
                .is_err(),
            "the overlay holds the reach lock through its write and apply"
        );

        let confirmed_ctx = ctx.clone();
        let mut confirmed = tokio::spawn(async move {
            confirmed_ctx
                .process(
                    DESTINATION,
                    &LogBatch {
                        from_block: 895_700,
                        to_block: 895_710,
                        direction: ScanDirection::Realtime,
                        logs: relay_logs(),
                    },
                )
                .await
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut confirmed)
                .await
                .is_err(),
            "the confirmed batch waits for the reach lock"
        );
        assert_eq!(
            ledger_lock_waiters(&db).await,
            1,
            "the confirmed batch waits for the reach lock, not inside its own write"
        );

        locker.rollback().await.unwrap();
        let report = task.await.unwrap();
        assert_eq!(report.processed_logs, relay_logs().len());
        confirmed
            .await
            .unwrap()
            .unwrap_or_else(|err| panic!("{:#}", err.error));

        assert_eq!(
            *ctx.progress[&DESTINATION].confirmed_reach.lock().await,
            895_710
        );
        let rows = ledger(&db).await;
        assert!(!rows.is_empty());
        for row in rows {
            assert_eq!(row.state, "confirmed", "{row:?}");
            assert!(row.confirmed_seen_at.is_some(), "{row:?}");
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_fetch_error_keeps_next_from() {
        let (_guard, db) = setup("op_overlay_loop_fetch_error").await;
        let mock = HeadMockService::new(895_800, vec![]).with_logs(vec![ScriptedLogs::Error]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState {
            next_from: Some(895_690),
        };

        let report = tick(&ctx, DESTINATION, &mut state).await;

        assert_eq!(report.read, None);
        assert_eq!(state.next_from, Some(895_690));
        assert!(ledger(&db).await.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_caps_the_range_at_latest_and_reads_nothing_above_it() {
        let (_guard, db) = setup("op_overlay_loop_cap_at_latest").await;
        let mock = HeadMockService::new(895_720, vec![]).with_logs(vec![empty()]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        let mut state = OverlayState {
            next_from: Some(895_690),
        };

        let report = tick(&ctx, DESTINATION, &mut state).await;
        assert_eq!(report.read, Some((895_690, 895_720)));
        assert_eq!(state.next_from, Some(895_721));

        let report = tick(&ctx, DESTINATION, &mut state).await;
        assert_eq!(report.read, None);
        assert_eq!(state.next_from, Some(895_721));
        assert_eq!(count(&mock, "eth_getLogs"), 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_on_still_sweeps() {
        let (_guard, db) = setup("op_overlay_loop_on_sweeps").await;
        let mock = HeadMockService::new(895_800, vec![]).with_logs(vec![empty()]);
        let ctx = overlay_context(&db, new_buffer(&db, Duration::from_secs(60)), &mock, true).await;
        feed_overlay(&ctx, SOURCE, send_batch()).await;
        ctx.progress[&SOURCE].raise_frontier(498_922);
        let mut state = OverlayState {
            next_from: Some(895_790),
        };

        let report = tick(&ctx, SOURCE, &mut state).await;

        assert_eq!(report.swept.map(|outcome| outcome.retracted), Some(1));
        all_rows_have_state(&db, "retracted").await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_overlay_off_only_sweeps() {
        let (_guard, db) = setup("op_overlay_loop_off_only_sweeps").await;
        let mock = HeadMockService::new(895_800, vec![]);
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = overlay_context(&db, buffer, &mock, false).await;
        feed_overlay(&ctx, SOURCE, send_batch()).await;
        ctx.progress[&SOURCE].raise_frontier(498_922);
        let mut state = OverlayState::default();

        let report = tick(&ctx, SOURCE, &mut state).await;

        assert!(mock.calls().is_empty(), "no RPC call at all");
        assert_eq!(report.swept.map(|outcome| outcome.retracted), Some(1));
        all_rows_have_state(&db, "retracted").await;
    }
}
