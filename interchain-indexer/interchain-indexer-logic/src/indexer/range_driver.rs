// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{collections::HashMap, sync::Arc};

use alloy::{network::Ethereum, providers::DynProvider, rpc::types::Filter};
use anyhow::bail;
use futures::{Stream, StreamExt};
use tonic::async_trait;

use crate::{
    indexer::{
        failure_ledger::{
            FailureLedger,
            interval::{BlockRange, FailedInterval},
            settings::FailureRetrySettings,
        },
        metrics,
        retry_scheduler::{RetryChunkOutcome, RetryScheduler, ScheduledRetryChunk},
    },
    log_stream::{LogBatch, ScanDirection, fetch_logs},
    secret::redact_urls,
};

/// Maximum length of a `reason` string persisted into `indexer_failures`. The
/// column is `TEXT`, but an unbounded `anyhow` error chain in a row read by
/// an API is not desirable.
const MAX_REASON_LEN: usize = 500;

/// A processing failure, optionally narrowed to the sub-ranges that actually
/// failed. `attributed` empty means "the whole yielded range" — this is what
/// makes narrowing opt-in: an existing `process_batch` returning
/// `anyhow::Result<()>` participates unchanged via the `From` impl below,
/// since `?` produces the wide record.
pub struct BatchError {
    pub error: anyhow::Error,
    pub attributed: Vec<BlockRange>,
}

impl From<anyhow::Error> for BatchError {
    fn from(error: anyhow::Error) -> Self {
        Self {
            error,
            attributed: vec![],
        }
    }
}

/// `#[async_trait]` rather than native AFIT because the `process` future is
/// awaited inside spawned indexer tasks and therefore must be `Send`.
#[async_trait]
pub trait RangeProcessor: Send + Sync {
    fn bridge_id(&self) -> i32;
    fn chain_ids(&self) -> Vec<i64>;
    fn provider(&self, chain_id: i64) -> Option<DynProvider<Ethereum>>;
    fn log_filter(&self, chain_id: i64) -> anyhow::Result<Filter>;
    /// The indexer's own configured block-range width, reused verbatim for
    /// replay. No retry-specific width exists.
    fn batch_size(&self) -> u64;

    async fn process(&self, chain_id: i64, batch: &LogBatch) -> Result<(), BatchError>;
}

/// `err.attributed`, or the whole yielded/retried range when empty.
fn attributed_ranges(err: &BatchError, whole_range: BlockRange) -> Vec<BlockRange> {
    if err.attributed.is_empty() {
        vec![whole_range]
    } else {
        err.attributed.clone()
    }
}

fn with_reason(ranges: Vec<BlockRange>, reason: String) -> Vec<(BlockRange, String)> {
    ranges
        .into_iter()
        .map(|range| (range, reason.clone()))
        .collect()
}

/// Truncate `reason` to [`MAX_REASON_LEN`] bytes on a `char` boundary so a
/// long `anyhow` error chain does not grow `indexer_failures.reason`
/// unboundedly.
fn truncate_reason(reason: &str) -> String {
    if reason.len() <= MAX_REASON_LEN {
        return reason.to_string();
    }

    let mut end = MAX_REASON_LEN;
    while end > 0 && !reason.is_char_boundary(end) {
        end -= 1;
    }
    reason[..end].to_string()
}

/// Shared loop replacing each indexer's hand-rolled `while let Some(batch) =
/// stream.next().await` — carries failure recording, replay, and escalation
/// for any `RangeProcessor`.
pub struct RangeDriver<P: RangeProcessor> {
    processor: P,
    ledger: Arc<FailureLedger>,
    settings: FailureRetrySettings,
}

impl<P: RangeProcessor> RangeDriver<P> {
    pub fn new(processor: P, ledger: Arc<FailureLedger>, settings: FailureRetrySettings) -> Self {
        Self {
            processor,
            ledger,
            settings,
        }
    }

    /// Drives one sequential handler future per chain, concurrently with the
    /// bridge-wide retry pass, until either every chain's stream ends
    /// (`Ok(())`) or the escalation path fires (`Err`).
    pub async fn run<S>(self, streams: Vec<(i64, S)>) -> anyhow::Result<()>
    where
        S: Stream<Item = (i64, LogBatch)> + Unpin + Send,
    {
        let bridge_id = self.processor.bridge_id();
        let pairs: Vec<(i32, i64)> = self
            .processor
            .chain_ids()
            .into_iter()
            .map(|chain_id| (bridge_id, chain_id))
            .collect();

        // ONE ledger, ONE initialize, over ALL pairs. `initialize` replaces
        // rather than merges its cache, so a second caller would wipe the
        // first's entries; there is never a second caller here.
        self.ledger.initialize(&pairs).await?;

        let mut retry_scheduler = RetryScheduler::new(
            self.processor.batch_size(),
            self.settings.split_after_attempts,
            self.settings.backoff_base,
            self.settings.backoff_cap,
        );

        // One sequential handler per chain. Within a chain, batches are
        // still processed strictly in arrival order — `handle_batch` is
        // awaited to completion before the next item is pulled — which is
        // what the cursor's gap-bridging depends on. Only ordering *across*
        // chains is relaxed.
        let this = &self;
        let chains = futures::future::try_join_all(streams.into_iter().map(
            move |(chain_id, mut stream)| async move {
                while let Some((stream_chain_id, batch)) = stream.next().await {
                    debug_assert_eq!(
                        stream_chain_id, chain_id,
                        "per-chain stream tagged with the wrong chain id"
                    );
                    this.handle_batch(bridge_id, chain_id, batch).await?;
                }
                tracing::warn!(bridge_id, chain_id, "per-chain log stream ended");
                anyhow::Ok(())
            },
        ));

        // A sibling future, not a `select!` branch: as a branch it ran to
        // completion with nothing else polled, so a replay pass blocked
        // every forward stream on the bridge for its whole duration.
        let retry = async {
            let mut retry_tick = tokio::time::interval(self.settings.scan_interval);
            // The default (Burst) would fire a run of catch-up ticks
            // immediately after a long retry pass.
            retry_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                retry_tick.tick().await;
                // Only the replay work is gated by the kill switch;
                // recording still happens when `enabled` is `false`
                // (README-documented). The failed-blocks / oldest-open-hole
                // gauges are no longer refreshed here: they are
                // config-scoped and refreshed by
                // `spawn_indexing_progress_metrics_worker`
                // (`interchain-indexer-server/src/server.rs`), which runs
                // regardless of whether any driver loop is alive — so a
                // pair whose indexer never started, or whose driver has
                // since escalated to `Failed`, still gets both series
                // instead of no series or a frozen one.
                if self.settings.enabled {
                    self.run_retry_tick(bridge_id, &pairs, &mut retry_scheduler)
                        .await;
                }
            }
        };

        tokio::select! {
            result = chains => result.map(|_| ()),
            // `retry` is an unconditional `loop` and cannot complete; this
            // arm exists only so that it is polled alongside the chain
            // handlers.
            () = retry => Ok(()),
        }
    }

    async fn handle_batch(
        &self,
        bridge_id: i32,
        chain_id: i64,
        batch: LogBatch,
    ) -> anyhow::Result<()> {
        let range = BlockRange {
            from: batch.from_block,
            to: batch.to_block,
        };

        match self.processor.process(chain_id, &batch).await {
            Ok(()) => {
                // No DB statement when the pair's cache entry says the set
                // is already empty — this is what keeps the healthy path
                // DB-free.
                if let Err(err) = self.ledger.resolve(bridge_id, chain_id, &[range]).await {
                    tracing::error!(
                        err = %redact_urls(&format!("{err:#}")),
                        bridge_id,
                        chain_id,
                        from_block = range.from,
                        to_block = range.to,
                        direction = ?batch.direction,
                        "failed to resolve a successfully processed range; it remains recorded and will be retried"
                    );
                }
            }
            Err(batch_err) => {
                let redacted_processing_error = redact_urls(&format!("{:#}", batch_err.error));
                let ranges = attributed_ranges(&batch_err, range);
                let reason = truncate_reason(&redacted_processing_error);
                let ranges_with_reason = with_reason(ranges, reason);

                match self
                    .ledger
                    .record(bridge_id, chain_id, &ranges_with_reason)
                    .await
                {
                    Ok(()) => {
                        tracing::error!(
                            err = %redacted_processing_error,
                            bridge_id,
                            chain_id,
                            from_block = range.from,
                            to_block = range.to,
                            direction = ?batch.direction,
                            "failed to process log batch; recorded for retry"
                        );
                    }
                    Err(first_err) => {
                        if let Err(final_err) = self
                            .retry_record(bridge_id, chain_id, &ranges_with_reason, first_err)
                            .await
                        {
                            metrics::FAILURE_RECORD_ESCALATIONS_TOTAL
                                .with_label_values(&[&bridge_id.to_string(), &chain_id.to_string()])
                                .inc();
                            // With no cursor barrier, `record()` is the last
                            // point where data can be permanently lost.
                            // Stopping closes that for every *subsequent*
                            // batch: realtime is monotone forward per chain,
                            // so once the driver stops consuming, no buffer
                            // entry above the failed interval can appear and
                            // the cursor cannot be derived past it.
                            //
                            // It does NOT close it for the batch in flight.
                            // Both adapters process a batch's transactions out
                            // of order and maintenance runs concurrently, so a
                            // later block may already be persisted — and the
                            // cursor already advanced past this failing one —
                            // before we get here. Stopping cannot retract
                            // that; closing it needs the acknowledgement
                            // boundary rejected in ADR-005, where it is
                            // carried as a known limitation.
                            let redacted_record_error = redact_urls(&format!("{final_err:#}"));
                            bail!(
                                "unable to record indexer failure for bridge {bridge_id} chain {chain_id} \
                                 range [{}, {}] after {} attempt(s) (processing error: {redacted_processing_error}): \
                                 {redacted_record_error}",
                                range.from,
                                range.to,
                                self.settings.record_retry_attempts,
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Retries `record` up to `record_retry_attempts` total attempts
    /// (including the one that already failed before this is called),
    /// doubling the delay from `record_retry_initial_backoff`.
    async fn retry_record(
        &self,
        bridge_id: i32,
        chain_id: i64,
        ranges_with_reason: &[(BlockRange, String)],
        first_error: anyhow::Error,
    ) -> anyhow::Result<()> {
        let mut last_err = first_error;
        let mut backoff = self.settings.record_retry_initial_backoff;

        for _ in 1..self.settings.record_retry_attempts {
            tokio::time::sleep(backoff).await;
            match self
                .ledger
                .record(bridge_id, chain_id, ranges_with_reason)
                .await
            {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_err = err;
                    backoff = backoff.saturating_mul(2);
                }
            }
        }

        Err(last_err)
    }

    async fn run_retry_tick(
        &self,
        bridge_id: i32,
        pairs: &[(i32, i64)],
        scheduler: &mut RetryScheduler,
    ) {
        self.run_retry_tick_at(bridge_id, pairs, scheduler, chrono::Utc::now().naive_utc())
            .await;
    }

    /// Crate-private deterministic seam for retry regression tests. The
    /// scheduler has no async dependencies; all provider, processor and ledger
    /// I/O stays here and no mutable scheduler borrow crosses an await.
    pub(crate) async fn run_retry_tick_at(
        &self,
        bridge_id: i32,
        pairs: &[(i32, i64)],
        scheduler: &mut RetryScheduler,
        decision_time: chrono::NaiveDateTime,
    ) {
        let open = match self.ledger.open(pairs).await {
            Ok(open) => open,
            Err(err) => {
                tracing::error!(err = %redact_urls(&format!("{err:#}")), bridge_id, "failed to query open indexer failures for retry pass");
                return;
            }
        };
        let rows: Vec<(i64, FailedInterval)> = open
            .into_iter()
            .map(|(_, chain_id, interval)| (chain_id, interval))
            .collect();
        let stats = scheduler.begin_tick(&rows, decision_time);
        if stats.ready_sessions > 0 {
            tracing::info!(
                bridge_id,
                open_sessions = rows.len(),
                tracked_sessions = stats.open_sessions,
                ready_sessions = stats.ready_sessions,
                max_chunks = self.settings.max_chunks_per_pass,
                "starting RETRY tick"
            );
        }

        let mut targets: HashMap<i64, Option<(DynProvider<Ethereum>, Filter)>> = HashMap::new();
        for _ in 0..self.settings.max_chunks_per_pass {
            let Some(chunk) = scheduler.next_chunk(decision_time) else {
                break;
            };
            let outcome = self.retry_chunk(bridge_id, chunk, &mut targets).await;
            if let Some(completion) =
                scheduler.report_outcome(chunk, outcome, chrono::Utc::now().naive_utc())
            {
                tracing::info!(bridge_id, session_id = completion.session_id, chain_id = completion.chain_id, old_width = completion.old_width, new_width = completion.new_width, progress = completion.resolved_any, failed_sweeps = completion.failed_sweeps, narrowing = completion.narrowing_started, next_due_at = ?completion.next_due_at, "completed RETRY sweep");
            }
        }
    }

    async fn retry_chunk(
        &self,
        bridge_id: i32,
        chunk: ScheduledRetryChunk,
        targets: &mut HashMap<i64, Option<(DynProvider<Ethereum>, Filter)>>,
    ) -> RetryChunkOutcome {
        let target = targets.entry(chunk.chain_id).or_insert_with(|| match (
            self.processor.provider(chunk.chain_id),
            self.processor.log_filter(chunk.chain_id),
        ) {
            (None, _) => {
                tracing::error!(bridge_id, chain_id = chunk.chain_id, "no provider configured for chain during retry tick");
                None
            }
            (Some(_), Err(err)) => {
                tracing::error!(err = %redact_urls(&format!("{err:#}")), bridge_id, chain_id = chunk.chain_id, "failed to build log filter during retry tick");
                None
            }
            (Some(provider), Ok(filter)) => Some((provider, filter)),
        });
        let Some((provider, filter)) = target.as_ref() else {
            return RetryChunkOutcome::NotResolved;
        };
        tracing::info!(
            bridge_id,
            chain_id = chunk.chain_id,
            from_block = chunk.range.from,
            to_block = chunk.range.to,
            size = chunk.range.width(),
            "scanning RETRY logs"
        );
        match fetch_logs(provider.clone(), filter, chunk.range.from, chunk.range.to).await {
            Ok(mut logs) => {
                logs.sort_by_key(|log| (log.block_number, log.log_index));
                let batch = LogBatch {
                    from_block: chunk.range.from,
                    to_block: chunk.range.to,
                    direction: ScanDirection::Retry,
                    logs,
                };
                match self.processor.process(chunk.chain_id, &batch).await {
                    Ok(()) => match self
                        .ledger
                        .resolve(bridge_id, chunk.chain_id, &[chunk.range])
                        .await
                    {
                        Ok(()) => RetryChunkOutcome::Resolved,
                        Err(err) => {
                            tracing::error!(err = %redact_urls(&format!("{err:#}")), bridge_id, chain_id = chunk.chain_id, from_block = chunk.range.from, to_block = chunk.range.to, "failed to resolve a successfully retried chunk");
                            RetryChunkOutcome::NotResolved
                        }
                    },
                    Err(batch_err) => {
                        let redacted_error = redact_urls(&format!("{:#}", batch_err.error));
                        tracing::warn!(
                            err = %redacted_error,
                            bridge_id,
                            chain_id = chunk.chain_id,
                            from_block = chunk.range.from,
                            to_block = chunk.range.to,
                            direction = ?batch.direction,
                            "retried chunk still failing"
                        );
                        let ranges_with_reason = with_reason(
                            attributed_ranges(&batch_err, chunk.range),
                            truncate_reason(&redacted_error),
                        );
                        if let Err(err) = self
                            .ledger
                            .record(bridge_id, chunk.chain_id, &ranges_with_reason)
                            .await
                        {
                            tracing::error!(err = %redact_urls(&format!("{err:#}")), bridge_id, chain_id = chunk.chain_id, from_block = chunk.range.from, to_block = chunk.range.to, "failed to re-record a still-failing retried chunk");
                        }
                        RetryChunkOutcome::NotResolved
                    }
                }
            }
            Err(err) => {
                let redacted_error = redact_urls(&format!("{err:#}"));
                tracing::warn!(err = %redacted_error, bridge_id, chain_id = chunk.chain_id, from_block = chunk.range.from, to_block = chunk.range.to, "failed to re-fetch a retried chunk");
                if let Err(record_err) = self
                    .ledger
                    .record(
                        bridge_id,
                        chunk.chain_id,
                        &[(chunk.range, truncate_reason(&redacted_error))],
                    )
                    .await
                {
                    tracing::error!(err = %redact_urls(&format!("{record_err:#}")), bridge_id, chain_id = chunk.chain_id, from_block = chunk.range.from, to_block = chunk.range.to, "failed to re-record a retry-fetch failure");
                }
                RetryChunkOutcome::NotResolved
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_error_from_anyhow_error_leaves_attributed_empty() {
        let err: BatchError = anyhow::anyhow!("boom").into();

        assert!(err.attributed.is_empty());
        assert_eq!(format!("{}", err.error), "boom");
    }

    #[test]
    fn truncate_reason_keeps_short_strings_unchanged() {
        assert_eq!(truncate_reason("short"), "short");
    }

    #[test]
    fn truncate_reason_caps_long_strings_at_the_bound() {
        let long = "a".repeat(1000);

        let truncated = truncate_reason(&long);

        assert_eq!(truncated.len(), MAX_REASON_LEN);
    }

    #[test]
    fn truncate_reason_does_not_split_a_multi_byte_char() {
        // Each 'é' is 2 bytes; pad so the naive byte cut at MAX_REASON_LEN
        // would land mid-character.
        let long = "é".repeat(MAX_REASON_LEN);

        let truncated = truncate_reason(&long);

        assert!(truncated.len() <= MAX_REASON_LEN);
        assert!(String::from_utf8(truncated.into_bytes()).is_ok());
    }

    #[test]
    fn attributed_ranges_falls_back_to_the_whole_range_when_empty() {
        let whole = BlockRange { from: 10, to: 20 };
        let err = BatchError {
            error: anyhow::anyhow!("boom"),
            attributed: vec![],
        };

        assert_eq!(attributed_ranges(&err, whole), vec![whole]);
    }

    #[test]
    fn attributed_ranges_uses_the_narrowed_set_when_present() {
        let whole = BlockRange { from: 10, to: 20 };
        let narrowed = vec![BlockRange { from: 12, to: 13 }];
        let err = BatchError {
            error: anyhow::anyhow!("boom"),
            attributed: narrowed.clone(),
        };

        assert_eq!(attributed_ranges(&err, whole), narrowed);
    }

    #[test]
    fn with_reason_pairs_every_range_with_a_clone_of_the_same_reason() {
        let ranges = vec![BlockRange { from: 1, to: 2 }, BlockRange { from: 5, to: 6 }];

        let paired = with_reason(ranges.clone(), "boom".to_string());

        assert_eq!(paired.len(), 2);
        assert!(paired.iter().all(|(_, reason)| reason == "boom"));
    }

    // --- DB-backed driver tests ---
    //
    // The "universality" case lives here rather than `indexer/example/`,
    // whose `ExampleIndexer` does not use `LogStream` at all. `TestRangeProcessor`
    // below is the minimal `RangeProcessor` that demonstrates recording and
    // replay with no protocol-specific code.

    mod db_tests {
        use std::{
            collections::{HashMap, HashSet, VecDeque},
            future,
            sync::atomic::{AtomicUsize, Ordering},
            task::{Context as TaskContext, Poll},
            time::Duration,
        };

        use alloy::{
            providers::{Provider, ProviderBuilder},
            rpc::{client::RpcClient, types::Log},
            transports::{TransportError, TransportErrorKind, TransportFut},
        };
        use alloy_json_rpc::{Id, RequestPacket, Response, ResponsePacket, ResponsePayload};
        use parking_lot::Mutex;
        use sea_orm::{ActiveValue, EntityTrait};
        use tower::Service;

        use super::super::*;
        use crate::{
            InterchainDatabase, MessageBufferSettings,
            message_buffer::{Consolidate, ConsolidatedMessage, Key, MessageBuffer},
            test_utils::{init_db, mock_db::fill_mock_interchain_database},
        };

        #[derive(Clone)]
        enum MockLogsAction {
            Logs(Vec<Log>),
            Error(&'static str),
            /// Never resolves. Used to park a caller (the retry pass's
            /// `fetch_logs`) inside the RPC call itself, the same place a
            /// genuinely hanging endpoint would park it.
            Block,
        }

        /// Minimal `eth_getLogs`-only mock transport: every call pops the next
        /// queued action, regardless of the request's method — sufficient
        /// because `fetch_logs` is the only caller of any provider built from
        /// this service in these tests.
        #[derive(Clone)]
        struct MockLogsService {
            actions: Arc<Mutex<VecDeque<MockLogsAction>>>,
        }

        impl MockLogsService {
            fn new() -> Self {
                Self {
                    actions: Arc::new(Mutex::new(VecDeque::new())),
                }
            }

            fn push_logs(&self, logs: Vec<Log>) {
                self.actions.lock().push_back(MockLogsAction::Logs(logs));
            }

            fn push_error(&self, msg: &'static str) {
                self.actions.lock().push_back(MockLogsAction::Error(msg));
            }

            fn push_block(&self) {
                self.actions.lock().push_back(MockLogsAction::Block);
            }
        }

        impl Service<RequestPacket> for MockLogsService {
            type Response = ResponsePacket;
            type Error = TransportError;
            type Future = TransportFut<'static>;

            fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
                Poll::Ready(Ok(()))
            }

            fn call(&mut self, req: RequestPacket) -> Self::Future {
                let action = self.actions.lock().pop_front();
                match action {
                    Some(MockLogsAction::Logs(logs)) => {
                        Box::pin(future::ready(Ok(build_logs_response(&req, &logs))))
                    }
                    Some(MockLogsAction::Error(msg)) => {
                        Box::pin(future::ready(Err(TransportErrorKind::custom_str(msg))))
                    }
                    Some(MockLogsAction::Block) => Box::pin(future::pending()),
                    None => Box::pin(future::ready(Err(TransportErrorKind::custom_str(
                        "no mock action queued",
                    )))),
                }
            }
        }

        fn build_logs_response(req: &RequestPacket, logs: &[Log]) -> ResponsePacket {
            let id = req
                .as_single()
                .map(|serialized| serialized.meta().id.clone())
                .unwrap_or_else(|| Id::Number(1));
            let payload = serde_json::value::to_raw_value(logs).expect("logs serialize");
            ResponsePacket::Single(Response {
                id,
                payload: ResponsePayload::Success(payload),
            })
        }

        fn mock_provider(service: MockLogsService) -> DynProvider<Ethereum> {
            let client = RpcClient::builder().transport(service, false);
            ProviderBuilder::new().connect_client(client).erased()
        }

        /// A minimal `RangeProcessor` adopting only the driver: no
        /// protocol-specific decoding, just a configurable pass/fail decision
        /// per exact `(chain_id, from_block, to_block)`. Everything else
        /// (recording, replay, escalation) comes from `RangeDriver`.
        struct TestRangeProcessor {
            bridge_id: i32,
            chain_ids: Vec<i64>,
            batch_size: u64,
            providers: HashMap<i64, DynProvider<Ethereum>>,
            provider_calls: Arc<Mutex<HashMap<i64, usize>>>,
            filter_calls: Arc<Mutex<HashMap<i64, usize>>>,
            fail_exact: Arc<Mutex<HashSet<(i64, u64, u64)>>>,
            process_calls: Arc<AtomicUsize>,
            /// Every `(chain_id, from_block, to_block)` ever passed to
            /// `process()`, in call order. Only the fairness/rotation test
            /// reads this; every other test ignores it.
            attempted: Arc<Mutex<Vec<(i64, u64, u64)>>>,
            /// Chains whose `process()` waits on `slow_gate` before
            /// returning. Models a chain deliberately throttled to a low
            /// `max_rps`: slow, but perfectly healthy and eventually
            /// successful.
            slow_chains: Arc<Mutex<HashSet<i64>>>,
            /// Released by the test once it has asserted sibling progress.
            slow_gate: Arc<tokio::sync::Notify>,
            /// Chains whose `process()` never returns at all.
            blocked_chains: Arc<Mutex<HashSet<i64>>>,
            /// Incremented on entry to `process()`, before any waiting.
            process_entries: Arc<AtomicUsize>,
            /// Every completed `(chain_id, from_block, to_block)`, reported
            /// as it happens so a test can assert progress *while* another
            /// chain is still working rather than on final totals.
            completed_tx: Option<tokio::sync::mpsc::UnboundedSender<(i64, u64, u64)>>,
        }

        impl TestRangeProcessor {
            fn new(bridge_id: i32, chain_ids: Vec<i64>, batch_size: u64) -> Self {
                Self {
                    bridge_id,
                    chain_ids,
                    batch_size,
                    providers: HashMap::new(),
                    provider_calls: Arc::new(Mutex::new(HashMap::new())),
                    filter_calls: Arc::new(Mutex::new(HashMap::new())),
                    fail_exact: Arc::new(Mutex::new(HashSet::new())),
                    process_calls: Arc::new(AtomicUsize::new(0)),
                    attempted: Arc::new(Mutex::new(Vec::new())),
                    slow_chains: Arc::new(Mutex::new(HashSet::new())),
                    slow_gate: Arc::new(tokio::sync::Notify::new()),
                    blocked_chains: Arc::new(Mutex::new(HashSet::new())),
                    process_entries: Arc::new(AtomicUsize::new(0)),
                    completed_tx: None,
                }
            }

            fn with_provider(mut self, chain_id: i64, provider: DynProvider<Ethereum>) -> Self {
                self.providers.insert(chain_id, provider);
                self
            }

            fn fail_range(&self, chain_id: i64, from: u64, to: u64) {
                self.fail_exact.lock().insert((chain_id, from, to));
            }

            /// Marks `chain_id` as deliberately throttled: its `process()`
            /// calls block on `slow_gate` until the test releases it.
            fn slow_chain(self, chain_id: i64) -> Self {
                self.slow_chains.lock().insert(chain_id);
                self
            }

            /// Marks `chain_id` as permanently stuck inside `process()`.
            fn block_chain(self, chain_id: i64) -> Self {
                self.blocked_chains.lock().insert(chain_id);
                self
            }

            fn with_completion_observer(
                mut self,
                tx: tokio::sync::mpsc::UnboundedSender<(i64, u64, u64)>,
            ) -> Self {
                self.completed_tx = Some(tx);
                self
            }
        }

        #[async_trait]
        impl RangeProcessor for TestRangeProcessor {
            fn bridge_id(&self) -> i32 {
                self.bridge_id
            }

            fn chain_ids(&self) -> Vec<i64> {
                self.chain_ids.clone()
            }

            fn provider(&self, chain_id: i64) -> Option<DynProvider<Ethereum>> {
                *self.provider_calls.lock().entry(chain_id).or_default() += 1;
                self.providers.get(&chain_id).cloned()
            }

            fn log_filter(&self, chain_id: i64) -> anyhow::Result<Filter> {
                *self.filter_calls.lock().entry(chain_id).or_default() += 1;
                Ok(Filter::default())
            }

            fn batch_size(&self) -> u64 {
                self.batch_size
            }

            async fn process(&self, chain_id: i64, batch: &LogBatch) -> Result<(), BatchError> {
                self.process_entries.fetch_add(1, Ordering::SeqCst);
                // Models `process_batch`'s per-transaction awaits (receipt +
                // block RPC, each behind the node's rate limiter). Without a
                // yield here the tests would run each handler to completion
                // by accident and prove nothing about interleaving.
                tokio::task::yield_now().await;

                // Read membership into a bool and drop the guard before
                // awaiting — never hold a `parking_lot::Mutex` guard across
                // an `.await` point.
                let is_blocked = self.blocked_chains.lock().contains(&chain_id);
                if is_blocked {
                    std::future::pending::<()>().await;
                }
                let is_slow = self.slow_chains.lock().contains(&chain_id);
                if is_slow {
                    self.slow_gate.notified().await;
                }

                self.process_calls.fetch_add(1, Ordering::SeqCst);
                self.attempted
                    .lock()
                    .push((chain_id, batch.from_block, batch.to_block));
                let should_fail =
                    self.fail_exact
                        .lock()
                        .contains(&(chain_id, batch.from_block, batch.to_block));

                if let Some(tx) = &self.completed_tx {
                    let _ = tx.send((chain_id, batch.from_block, batch.to_block));
                }

                if should_fail {
                    Err(anyhow::anyhow!(
                        "synthetic failure for chain {chain_id} [{}, {}]",
                        batch.from_block,
                        batch.to_block
                    )
                    .into())
                } else {
                    Ok(())
                }
            }
        }

        fn empty_batch(from_block: u64, to_block: u64) -> LogBatch {
            LogBatch {
                from_block,
                to_block,
                direction: ScanDirection::Realtime,
                logs: vec![],
            }
        }

        fn retry_settings(max_chunks_per_pass: usize) -> FailureRetrySettings {
            FailureRetrySettings {
                max_chunks_per_pass,
                ..Default::default()
            }
        }

        fn retry_scheduler(batch_size: u64, settings: &FailureRetrySettings) -> RetryScheduler {
            RetryScheduler::new(
                batch_size,
                settings.split_after_attempts,
                settings.backoff_base,
                settings.backoff_cap,
            )
        }

        fn retry_decision_time() -> chrono::NaiveDateTime {
            chrono::NaiveDate::from_ymd_opt(2100, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
        }

        /// The healthy path performs zero ledger database statements: this is
        /// what keeps steady-state indexing DB-free. `resolve` on a pair with
        /// no cached holes returns before issuing any SQL, so the
        /// `FAILURE_LEDGER_WRITES_TOTAL{operation="resolve"}` counter for a
        /// bridge_id no other test can write to must stay at zero.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn healthy_path_issues_no_ledger_write_statement() {
            const SENTINEL_BRIDGE_ID: i32 = 910_001;

            let db = init_db("range_driver_healthy_path_no_ledger_write").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let processor = TestRangeProcessor::new(SENTINEL_BRIDGE_ID, vec![1], 1000);
            let settings = FailureRetrySettings {
                enabled: false,
                ..Default::default()
            };

            let stream = futures::stream::iter(vec![(1i64, empty_batch(1, 10))]);
            RangeDriver::new(processor, ledger, settings)
                .run(vec![(1i64, stream)])
                .await
                .unwrap();

            let writes = metrics::FAILURE_LEDGER_WRITES_TOTAL
                .with_label_values(&[&SENTINEL_BRIDGE_ID.to_string(), "resolve"])
                .get();
            assert_eq!(
                writes, 0,
                "healthy path must issue zero ledger database statements"
            );
        }

        /// Universality: a minimal `RangeProcessor` gets recording and replay
        /// through the forward path alone, with no protocol-specific code.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn records_a_failed_batch_and_resolves_it_on_a_later_success() {
            let db = init_db("range_driver_records_and_resolves_forward_path").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db.clone()));

            let processor = TestRangeProcessor::new(1, vec![1], 1000);
            processor.fail_range(1, 1, 10);

            let settings = FailureRetrySettings {
                enabled: false,
                ..Default::default()
            };

            let failing_batch = empty_batch(1, 10);
            // A later, wider batch covering the same range succeeds.
            let recovering_batch = empty_batch(1, 20);
            let stream =
                futures::stream::iter(vec![(1i64, failing_batch), (1i64, recovering_batch)]);

            RangeDriver::new(processor, ledger, settings)
                .run(vec![(1i64, stream)])
                .await
                .unwrap();

            let open = interchain_db
                .open_indexer_failures(&[(1, 1)])
                .await
                .unwrap();
            assert!(
                open.is_empty(),
                "the hole must be resolved once a covering range succeeds: {open:?}"
            );
        }

        /// With no cursor barrier, `record()` is the last point where data
        /// can be permanently lost. An unrecordable failure (here, a
        /// foreign-key violation on an unconfigured chain_id) must stop the
        /// driver — it must not request the next batch.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn escalates_and_stops_consuming_when_record_keeps_failing() {
            const UNSEEDED_CHAIN_ID: i64 = 999_999_999;

            let db = init_db("range_driver_escalates_on_unrecordable_failure").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let processor = TestRangeProcessor::new(1, vec![UNSEEDED_CHAIN_ID], 1000);
            processor.fail_range(UNSEEDED_CHAIN_ID, 1, 10);
            let process_calls = processor.process_calls.clone();

            let settings = FailureRetrySettings {
                enabled: false,
                record_retry_attempts: 2,
                record_retry_initial_backoff: Duration::from_millis(1),
                ..Default::default()
            };

            let batch1 = (UNSEEDED_CHAIN_ID, empty_batch(1, 10));
            let batch2 = (UNSEEDED_CHAIN_ID, empty_batch(11, 20));
            let stream = futures::stream::iter(vec![batch1, batch2]);

            let result = RangeDriver::new(processor, ledger, settings)
                .run(vec![(UNSEEDED_CHAIN_ID, stream)])
                .await;

            assert!(result.is_err(), "an unrecordable failure must escalate");
            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                1,
                "the driver must not request the next batch after escalating"
            );
        }

        /// PRIMARY ACCEPTANCE TEST for per-chain range drivers.
        ///
        /// The delay is injected inside `RangeProcessor::process`, NOT inside
        /// the stream, and that is load-bearing. A stream-level stall is the
        /// already-working case: `SelectAll` (the pre-change merge strategy)
        /// leaves a `Pending` stream alone and polls its siblings. Measured
        /// 2026-08-21, black-holing one chain's RPC made the others speed up
        /// (C-Chain catchup 33 -> 241.5 blocks/s), so a test that stalls the
        /// stream passes against the pre-change code and proves nothing. The
        /// real defect is wall-clock spent inside `process` — which is where
        /// a chain throttled to a low `max_rps` spends its time, waiting at
        /// `node.limiter.until_ready()` once per receipt and once per block.
        /// Do not "simplify" this into a stream-level stall.
        ///
        /// The assertion is progress made *while* the slow chain is still
        /// working, never a final total: a final-total assertion passes on
        /// the pre-change sequential code too, once it eventually gets
        /// there.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn a_slow_chain_does_not_slow_down_its_siblings() {
            const BRIDGE_ID: i32 = 1;
            const SLOW_CHAIN: i64 = 1;
            const FAST_CHAIN: i64 = 2;
            const FAST_BATCHES: usize = 10;

            let db = init_db("range_driver_slow_chain_does_not_slow_siblings").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let processor = TestRangeProcessor::new(BRIDGE_ID, vec![SLOW_CHAIN, FAST_CHAIN], 1000)
                .slow_chain(SLOW_CHAIN)
                .with_completion_observer(tx);
            let process_entries = processor.process_entries.clone();
            let process_calls = processor.process_calls.clone();
            let attempted = processor.attempted.clone();
            let slow_gate = processor.slow_gate.clone();

            let settings = FailureRetrySettings {
                enabled: false,
                ..Default::default()
            };

            // A single, long-but-finite batch: the slow chain's process()
            // call blocks on the gate until the test releases it below.
            let slow_stream = futures::stream::iter(vec![(SLOW_CHAIN, empty_batch(1, 10))]);
            let fast_batches: Vec<(i64, LogBatch)> = (0..FAST_BATCHES)
                .map(|i| {
                    let from = (i as u64) * 10 + 1;
                    (FAST_CHAIN, empty_batch(from, from + 9))
                })
                .collect();
            let fast_stream = futures::stream::iter(fast_batches);

            let mut driver = Box::pin(
                RangeDriver::new(processor, ledger.clone(), settings)
                    .run(vec![(SLOW_CHAIN, slow_stream), (FAST_CHAIN, fast_stream)]),
            );

            let outcome = tokio::time::timeout(Duration::from_secs(5), async {
                let mut fast_completions = Vec::new();
                tokio::select! {
                    result = &mut driver => panic!("driver returned early: {result:?}"),
                    () = async {
                        while fast_completions.len() < FAST_BATCHES {
                            let completed = rx.recv().await.expect("observer channel");
                            assert_eq!(
                                completed.0, FAST_CHAIN,
                                "the slow chain must not have completed a batch yet"
                            );
                            fast_completions.push(completed);
                        }
                    } => {}
                }
                fast_completions
            })
            .await;

            let fast_completions = outcome.expect(
                "the fast chain made no progress while its sibling was still inside \
                 process(): head-of-line blocking between the chains of one bridge is back",
            );

            assert_eq!(fast_completions.len(), FAST_BATCHES);
            let mut previous_from_block = None;
            for (chain_id, from_block, _) in &fast_completions {
                assert_eq!(*chain_id, FAST_CHAIN);
                if let Some(previous) = previous_from_block {
                    assert!(
                        *from_block > previous,
                        "fast chain batches must complete in ascending order"
                    );
                }
                previous_from_block = Some(*from_block);
            }

            // The slow chain must have entered `process()` exactly once
            // (alongside every fast batch's own entry) and gotten past the
            // gate zero times.
            assert_eq!(
                process_entries.load(Ordering::SeqCst),
                FAST_BATCHES + 1,
                "the slow chain must have entered process() exactly once while parked"
            );
            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                FAST_BATCHES,
                "the slow chain must not have gotten past the gate yet"
            );

            // A slow chain is healthy: releasing it must let it finish
            // exactly like a serial run would, only later. `notify_one`
            // stores a permit if nobody is waiting yet, so this is safe
            // regardless of exactly when the slow chain's `process()`
            // reaches its `.await` point.
            slow_gate.notify_one();

            let final_result = tokio::time::timeout(Duration::from_secs(5), driver)
                .await
                .expect("the driver must complete once the slow chain is released");
            assert!(
                final_result.is_ok(),
                "a merely slow chain must not escalate the driver: {final_result:?}"
            );

            assert_eq!(
                attempted
                    .lock()
                    .iter()
                    .filter(|(chain_id, _, _)| *chain_id == SLOW_CHAIN)
                    .count(),
                1,
                "the slow chain's batch must complete normally once released"
            );
            let open = ledger.open(&[(BRIDGE_ID, SLOW_CHAIN)]).await.unwrap();
            assert!(
                open.is_empty(),
                "a merely slow chain must never be marked failed: {open:?}"
            );
        }

        /// Secondary acceptance case: the degenerate extreme of the same
        /// property, kept separate because it needs no gate release. `run`
        /// never returns here — the blocked chain's handler is parked
        /// forever — which is correct, hence driving it inside `select!`
        /// against the observer rather than awaiting it directly.
        ///
        /// Same load-bearing requirement as the primary test: the delay is
        /// injected inside `RangeProcessor::process`, never inside the
        /// stream. See `a_slow_chain_does_not_slow_down_its_siblings` for why
        /// a stream-level stall would prove nothing here.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn a_chain_blocked_inside_process_does_not_stop_its_siblings() {
            const BRIDGE_ID: i32 = 1;
            const BLOCKED_CHAIN: i64 = 1;
            const FAST_CHAIN: i64 = 2;
            const FAST_BATCHES: usize = 10;

            let db = init_db("range_driver_blocked_chain_does_not_stop_siblings").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let processor =
                TestRangeProcessor::new(BRIDGE_ID, vec![BLOCKED_CHAIN, FAST_CHAIN], 1000)
                    .block_chain(BLOCKED_CHAIN)
                    .with_completion_observer(tx);
            let process_entries = processor.process_entries.clone();
            let process_calls = processor.process_calls.clone();

            let settings = FailureRetrySettings {
                enabled: false,
                ..Default::default()
            };

            let blocked_stream = futures::stream::iter(vec![(BLOCKED_CHAIN, empty_batch(1, 10))]);
            let fast_batches: Vec<(i64, LogBatch)> = (0..FAST_BATCHES)
                .map(|i| {
                    let from = (i as u64) * 10 + 1;
                    (FAST_CHAIN, empty_batch(from, from + 9))
                })
                .collect();
            let fast_stream = futures::stream::iter(fast_batches);

            let mut driver = Box::pin(RangeDriver::new(processor, ledger, settings).run(vec![
                (BLOCKED_CHAIN, blocked_stream),
                (FAST_CHAIN, fast_stream),
            ]));

            let outcome = tokio::time::timeout(Duration::from_secs(5), async {
                let mut fast_completions = Vec::new();
                tokio::select! {
                    result = &mut driver => panic!("driver returned early: {result:?}"),
                    () = async {
                        while fast_completions.len() < FAST_BATCHES {
                            let completed = rx.recv().await.expect("observer channel");
                            assert_eq!(
                                completed.0, FAST_CHAIN,
                                "the blocked chain must never complete a batch"
                            );
                            fast_completions.push(completed);
                        }
                    } => {}
                }
                fast_completions
            })
            .await;

            let fast_completions = outcome.expect(
                "the fast chain made no progress while its sibling was permanently stuck \
                 inside process(): head-of-line blocking between the chains of one bridge \
                 is back",
            );

            assert_eq!(fast_completions.len(), FAST_BATCHES);
            assert_eq!(
                process_entries.load(Ordering::SeqCst),
                FAST_BATCHES + 1,
                "the blocked chain must have entered process() exactly once"
            );
            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                FAST_BATCHES,
                "the blocked chain must never get past its (never-returning) process() call"
            );
        }

        /// Mandatory scope item 2: the retry pass runs as a sibling future,
        /// not a `select!` branch awaited inline — so a replay pass parked
        /// inside `fetch_logs` must not stop the forward streams. Against
        /// the pre-change code the retry pass owns the `select!` arm and the
        /// forward stream is never polled, so this test times out with a
        /// clear failure message.
        ///
        /// The forward chain's stream deliberately carries more batches than
        /// the observed threshold: unlike the two acceptance tests above,
        /// nothing else keeps `try_join_all` from completing on its own once
        /// the forward stream is exhausted (the retry pass is not one of its
        /// members), so exhausting it exactly when the observer reaches its
        /// target count would race the driver's own completion against the
        /// observer inside the same `select!`. Leaving headroom avoids that.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn a_blocked_retry_pass_does_not_stop_the_forward_streams() {
            const BRIDGE_ID: i32 = 1;
            const RETRY_CHAIN: i64 = 1;
            const FORWARD_CHAIN: i64 = 2;
            const FORWARD_BATCHES: usize = 10;
            const FORWARD_STREAM_LEN: usize = 30;

            let db = init_db("range_driver_blocked_retry_pass_does_not_stop_forward").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            interchain_db
                .record_indexer_failures(
                    BRIDGE_ID,
                    RETRY_CHAIN,
                    &[(BlockRange { from: 0, to: 99 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let retry_mock = MockLogsService::new();
            retry_mock.push_block();

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
            let processor =
                TestRangeProcessor::new(BRIDGE_ID, vec![RETRY_CHAIN, FORWARD_CHAIN], 100)
                    .with_provider(RETRY_CHAIN, mock_provider(retry_mock))
                    .with_completion_observer(tx);

            let settings = FailureRetrySettings {
                enabled: true,
                scan_interval: Duration::from_millis(10),
                // Without this the test is vacuous. `is_due` gates a replay on
                // `last_attempt_at + backoff_base * 2^(attempts - 1)`, and the
                // interval recorded above lands with `attempts = 1` and
                // `last_attempt_at = now`. At the 30 s default the pass finds
                // nothing due for the whole 5 s window, never reaches
                // `fetch_logs`, and therefore never blocks — so the assertions
                // below hold even when the retry pass DOES starve the forward
                // streams. Make the interval due on the first tick instead.
                backoff_base: Duration::from_millis(1),
                ..Default::default()
            };

            let forward_batches: Vec<(i64, LogBatch)> = (0..FORWARD_STREAM_LEN)
                .map(|i| {
                    let from = (i as u64) * 10 + 1;
                    (FORWARD_CHAIN, empty_batch(from, from + 9))
                })
                .collect();
            let forward_stream = futures::stream::iter(forward_batches);

            // `RETRY_CHAIN` intentionally has no forward stream of its own:
            // this test is only about whether the retry pass (which does
            // still cover `RETRY_CHAIN`, via `chain_ids()`/`pairs`) can stop
            // `FORWARD_CHAIN`'s forward stream from progressing.
            let mut driver = Box::pin(
                RangeDriver::new(processor, ledger, settings)
                    .run(vec![(FORWARD_CHAIN, forward_stream)]),
            );

            let outcome = tokio::time::timeout(Duration::from_secs(5), async {
                let mut completions = Vec::new();
                tokio::select! {
                    result = &mut driver => panic!("driver returned early: {result:?}"),
                    () = async {
                        while completions.len() < FORWARD_BATCHES {
                            let completed = rx.recv().await.expect("observer channel");
                            assert_eq!(completed.0, FORWARD_CHAIN);
                            completions.push(completed);
                        }
                    } => {}
                }
                completions
            })
            .await;

            let completions = outcome.expect(
                "the forward chain made no progress while the retry pass was blocked inside \
                 fetch_logs: the retry pass starving the forward streams is back",
            );
            assert_eq!(completions.len(), FORWARD_BATCHES);
        }

        /// Invariant I7: an unrecordable failure on one chain must still
        /// fail the whole bridge, even though the chains now index
        /// independently. The healthy chain having processed at least one
        /// batch is what proves the two chains actually ran concurrently
        /// rather than sequentially before the escalation.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn an_unrecordable_failure_on_one_chain_still_fails_the_whole_bridge() {
            const BRIDGE_ID: i32 = 1;
            const UNSEEDED_CHAIN_ID: i64 = 999_999_998;
            const HEALTHY_CHAIN: i64 = 2;

            let db = init_db("range_driver_unrecordable_failure_fails_whole_bridge").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let processor =
                TestRangeProcessor::new(BRIDGE_ID, vec![UNSEEDED_CHAIN_ID, HEALTHY_CHAIN], 1000);
            processor.fail_range(UNSEEDED_CHAIN_ID, 1, 10);
            let attempted = processor.attempted.clone();

            let settings = FailureRetrySettings {
                enabled: false,
                record_retry_attempts: 2,
                record_retry_initial_backoff: Duration::from_millis(1),
                ..Default::default()
            };

            let failing_stream =
                futures::stream::iter(vec![(UNSEEDED_CHAIN_ID, empty_batch(1, 10))]);
            let healthy_batches: Vec<(i64, LogBatch)> = (0..5)
                .map(|i| {
                    let from = (i as u64) * 10 + 1;
                    (HEALTHY_CHAIN, empty_batch(from, from + 9))
                })
                .collect();
            let healthy_stream = futures::stream::iter(healthy_batches);

            let result = RangeDriver::new(processor, ledger, settings)
                .run(vec![
                    (UNSEEDED_CHAIN_ID, failing_stream),
                    (HEALTHY_CHAIN, healthy_stream),
                ])
                .await;

            assert!(
                result.is_err(),
                "an unrecordable failure on one chain must escalate the whole bridge"
            );
            let healthy_processed = attempted
                .lock()
                .iter()
                .filter(|(chain_id, _, _)| *chain_id == HEALTHY_CHAIN)
                .count();
            assert!(
                healthy_processed >= 1,
                "the healthy chain must have processed at least one batch, proving the \
                 two chains ran concurrently rather than sequentially"
            );
        }

        /// Invariant I2: within a chain, batches are still processed
        /// strictly in arrival order — only ordering *across* chains is
        /// relaxed by the per-chain driver split. Cross-chain interleaving
        /// in `attempted` is expected and must not be asserted against.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn per_chain_batches_are_processed_in_arrival_order() {
            const BRIDGE_ID: i32 = 1;
            const CHAIN_A: i64 = 1;
            const CHAIN_B: i64 = 2;

            let db = init_db("range_driver_per_chain_batches_arrival_order").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            let ledger = Arc::new(FailureLedger::new(interchain_db));

            let processor = TestRangeProcessor::new(BRIDGE_ID, vec![CHAIN_A, CHAIN_B], 1000);
            let attempted = processor.attempted.clone();

            let settings = FailureRetrySettings {
                enabled: false,
                ..Default::default()
            };

            let chain_a_batches: Vec<(i64, LogBatch)> = (0..8)
                .map(|i| {
                    let from = (i as u64) * 10 + 1;
                    (CHAIN_A, empty_batch(from, from + 9))
                })
                .collect();
            let chain_b_batches: Vec<(i64, LogBatch)> = (0..8)
                .map(|i| {
                    let from = (i as u64) * 20 + 1;
                    (CHAIN_B, empty_batch(from, from + 19))
                })
                .collect();

            let stream_a = futures::stream::iter(chain_a_batches);
            let stream_b = futures::stream::iter(chain_b_batches);

            RangeDriver::new(processor, ledger, settings)
                .run(vec![(CHAIN_A, stream_a), (CHAIN_B, stream_b)])
                .await
                .unwrap();

            let attempted = attempted.lock();
            for chain_id in [CHAIN_A, CHAIN_B] {
                let per_chain: Vec<u64> = attempted
                    .iter()
                    .filter(|(id, _, _)| *id == chain_id)
                    .map(|(_, from, _)| *from)
                    .collect();
                let mut sorted = per_chain.clone();
                sorted.sort_unstable();
                assert_eq!(
                    per_chain, sorted,
                    "chain {chain_id}'s batches must be processed in ascending from_block order"
                );
            }
        }

        /// The forward path never yields empty ranges, so a retried chunk
        /// that returns zero logs is the only place this case exists —
        /// getting it wrong means a hole that can never clear.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_pass_resolves_a_chunk_that_returns_zero_logs() {
            let db = init_db("range_driver_retry_resolves_zero_logs_chunk").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 100, to: 199 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();

            let mock_service = MockLogsService::new();
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));
            let settings = retry_settings(16);
            let mut scheduler = retry_scheduler(100, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);
            driver
                .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                .await;

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert!(
                open.is_empty(),
                "a retried chunk returning zero logs must still resolve: {open:?}"
            );
        }

        /// Chunking is mandatory: a recorded interval wider than `batch_size`
        /// must replay as multiple requests, and a failure on one chunk must
        /// leave only that chunk's remainder in the ledger — proof that a
        /// large hole converges instead of restarting.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_pass_chunking_leaves_only_the_failing_remainder() {
            let db = init_db("range_driver_retry_chunking_leaves_remainder").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 0, to: 2999 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();

            let mock_service = MockLogsService::new();
            // Three 1000-block chunks; all fetch successfully.
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 1000)
                .with_provider(1, mock_provider(mock_service));
            // The middle chunk fails at the `process()` level.
            processor.fail_range(1, 1000, 1999);
            let settings = retry_settings(16);
            let mut scheduler = retry_scheduler(1000, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);
            driver
                .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                .await;

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert_eq!(open.len(), 1);
            assert_eq!(
                open[0].2.range,
                BlockRange {
                    from: 1000,
                    to: 1999
                },
                "only the failing chunk should remain: {open:?}"
            );
        }

        /// `max_chunks_per_pass` bounds chunks per tick so a large hole set
        /// cannot starve the realtime scan sharing this task. A fresh resume
        /// cursor starts this pass's window at the interval head, so the two
        /// budgeted chunks (`[0,999]`, `[1000,1999]`) are the ones attempted;
        /// both resolve, leaving the untouched tail `[2000,4999]` as the single
        /// remaining open row.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_pass_is_bounded_by_max_chunks_per_pass() {
            let db = init_db("range_driver_retry_bounded_by_max_chunks").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            // Five 1000-block chunks.
            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 0, to: 4999 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();

            let mock_service = MockLogsService::new();
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 1000)
                .with_provider(1, mock_provider(mock_service));
            let process_calls = processor.process_calls.clone();
            let settings = retry_settings(2);
            let mut scheduler = retry_scheduler(1000, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);
            driver
                .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                .await;

            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                2,
                "exactly max_chunks_per_pass chunks must be attempted"
            );

            let mut open: Vec<BlockRange> = ledger
                .open(&[(1, 1)])
                .await
                .unwrap()
                .into_iter()
                .map(|(_, _, interval)| interval.range)
                .collect();
            open.sort_by_key(|range| range.from);

            // A fresh cursor starts at the head, so the first two chunks
            // (`[0,999]`, `[1000,1999]`) resolved and the untouched tail
            // remains as one row. The *next* pass would resume at `[2000,…]`;
            // that continuation is what
            // `retry_pass_fairness_reaches_chunks_beyond_a_permanently_failing_prefix`
            // covers.
            assert_eq!(
                open,
                vec![BlockRange {
                    from: 2000,
                    to: 4999
                }]
            );
        }

        /// Regression for the P2 "permanently failing prefix starves the
        /// tail" finding: with every retry pass starting at the queue head, a
        /// leading prefix wider than `max_chunks_per_pass` re-merged into the
        /// same still-open row on every tick and later chunks of that interval
        /// were never attempted at all. Ten 100-block chunks, the first four
        /// (blocks `[0, 399]`) permanently fail, and the budget is two chunks
        /// per pass — strictly narrower than the failing prefix, so a driver
        /// that does not carry a resume cursor loops on `[0,199]` forever.
        ///
        /// This is the end-to-end half of the guarantee; deterministic
        /// scheduler tests pin the sweep's completeness without a database.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_pass_fairness_reaches_chunks_beyond_a_permanently_failing_prefix() {
            const MAX_CHUNKS_PER_PASS: usize = 2;
            const MAX_PASSES: usize = 30;

            let db = init_db("range_driver_retry_fairness_beyond_failing_prefix").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 0, to: 999 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();

            let mock_service = MockLogsService::new();
            for _ in 0..(MAX_CHUNKS_PER_PASS * MAX_PASSES) {
                mock_service.push_logs(vec![]);
            }

            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));
            for from in [0u64, 100, 200, 300] {
                processor.fail_range(1, from, from + 99);
            }
            let attempted = processor.attempted.clone();
            let settings = retry_settings(MAX_CHUNKS_PER_PASS);
            let mut scheduler = retry_scheduler(100, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);

            let mut reached_beyond_prefix = false;

            for _pass in 0..MAX_PASSES {
                driver
                    .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                    .await;

                if attempted.lock().iter().any(|&(_, from, _)| from >= 400) {
                    reached_beyond_prefix = true;
                    break;
                }
            }

            assert!(
                reached_beyond_prefix,
                "chunks beyond the permanently-failing prefix (blocks [0, 399]) must \
                 eventually be attempted once that prefix is wider than \
                 max_chunks_per_pass; attempted so far: {:?}",
                attempted.lock()
            );
        }

        /// An `eth_getLogs` failure during retry re-records the chunk as
        /// still-failed and moves on — it is not an escalation, the range
        /// was never re-scanned.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_pass_fetch_failure_re_records_the_chunk_without_escalating() {
            let db = init_db("range_driver_retry_fetch_failure_re_records").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));

            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 100, to: 199 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();

            let mock_service = MockLogsService::new();
            mock_service.push_error("rpc unavailable");

            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));
            let settings = retry_settings(16);
            let mut scheduler = retry_scheduler(100, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);
            driver
                .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                .await;

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert_eq!(
                open.len(),
                1,
                "an eth_getLogs failure must re-record, not drop, the chunk"
            );
            assert_eq!(open[0].2.range, BlockRange { from: 100, to: 199 });
            assert_eq!(
                open[0].2.attempts, 2,
                "re-recording after a fetch failure still counts as another attempt"
            );
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_ticks_fetch_the_exact_adaptive_range_sequence() {
            let db = init_db("range_driver_retry_exact_adaptive_sequence").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 0, to: 7 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();
            let mock_service = MockLogsService::new();
            for _ in 0..4 {
                mock_service.push_logs(vec![]);
            }
            let processor = TestRangeProcessor::new(1, vec![1], 8)
                .with_provider(1, mock_provider(mock_service));
            for (from, to) in [(0, 7), (0, 3), (4, 7)] {
                processor.fail_range(1, from, to);
            }
            let attempted = processor.attempted.clone();
            let settings = retry_settings(8);
            let mut scheduler = retry_scheduler(8, &settings);
            let driver = RangeDriver::new(processor, ledger, settings);

            for _ in 0..3 {
                driver
                    .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                    .await;
            }

            assert_eq!(
                *attempted.lock(),
                vec![(1, 0, 7), (1, 0, 7), (1, 0, 3), (1, 4, 7)]
            );
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn missing_target_consumes_budget_without_starving_another_chain() {
            let db = init_db("range_driver_retry_missing_target_fairness").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            for chain_id in [1, 100] {
                interchain_db
                    .record_indexer_failures(
                        1,
                        chain_id,
                        &[(BlockRange { from: 0, to: 0 }, "boom".to_string())],
                    )
                    .await
                    .unwrap();
            }

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            let pairs = [(1, 1), (1, 100)];
            ledger.initialize(&pairs).await.unwrap();
            let mock_service = MockLogsService::new();
            mock_service.push_logs(vec![]);
            let processor = TestRangeProcessor::new(1, vec![1, 100], 1)
                .with_provider(100, mock_provider(mock_service));
            let attempted = processor.attempted.clone();
            let settings = retry_settings(2);
            let mut scheduler = retry_scheduler(1, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);

            driver
                .run_retry_tick_at(1, &pairs, &mut scheduler, retry_decision_time())
                .await;

            assert_eq!(*attempted.lock(), vec![(100, 0, 0)]);
            let open = ledger.open(&pairs).await.unwrap();
            assert_eq!(open.len(), 1);
            assert_eq!(
                (open[0].1, open[0].2.range),
                (1, BlockRange { from: 0, to: 0 })
            );
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn retry_tick_resolves_provider_and_filter_once_per_chain() {
            let db = init_db("range_driver_retry_target_cache_per_chain").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 0, to: 2 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();
            let mock_service = MockLogsService::new();
            for _ in 0..3 {
                mock_service.push_logs(vec![]);
            }
            let processor = TestRangeProcessor::new(1, vec![1], 1)
                .with_provider(1, mock_provider(mock_service));
            let provider_calls = processor.provider_calls.clone();
            let filter_calls = processor.filter_calls.clone();
            let settings = retry_settings(3);
            let mut scheduler = retry_scheduler(1, &settings);
            let driver = RangeDriver::new(processor, ledger, settings);

            driver
                .run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time())
                .await;

            assert_eq!(provider_calls.lock().get(&1), Some(&1));
            assert_eq!(filter_calls.lock().get(&1), Some(&1));
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn cancellation_during_retry_fetch_keeps_ledger_coverage() {
            let db = init_db("range_driver_retry_cancellation_keeps_coverage").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 100, to: 199 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            ledger.initialize(&[(1, 1)]).await.unwrap();
            let mock_service = MockLogsService::new();
            mock_service.push_block();
            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));
            let settings = retry_settings(1);
            let mut scheduler = retry_scheduler(100, &settings);
            let driver = RangeDriver::new(processor, ledger.clone(), settings);

            let result = tokio::time::timeout(
                Duration::from_millis(20),
                driver.run_retry_tick_at(1, &[(1, 1)], &mut scheduler, retry_decision_time()),
            )
            .await;
            assert!(
                result.is_err(),
                "mock fetch must remain pending until cancelled"
            );

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert_eq!(open.len(), 1);
            assert_eq!(open[0].2.range, BlockRange { from: 100, to: 199 });
        }

        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn disabled_retry_keeps_existing_ledger_rows_unchanged() {
            let db = init_db("range_driver_disabled_retry_keeps_coverage").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = Arc::new(InterchainDatabase::new(db.client()));
            interchain_db
                .record_indexer_failures(
                    1,
                    1,
                    &[(BlockRange { from: 5, to: 9 }, "boom".to_string())],
                )
                .await
                .unwrap();

            let ledger = Arc::new(FailureLedger::new(interchain_db));
            let processor = TestRangeProcessor::new(1, vec![1], 5);
            let settings = FailureRetrySettings {
                enabled: false,
                scan_interval: Duration::from_millis(1),
                ..Default::default()
            };
            let driver = RangeDriver::new(processor, ledger.clone(), settings);
            let pending = futures::stream::pending::<(i64, LogBatch)>();

            let result =
                tokio::time::timeout(Duration::from_millis(20), driver.run(vec![(1, pending)]))
                    .await;
            assert!(
                result.is_err(),
                "driver must still be waiting on the stream"
            );

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert_eq!(open.len(), 1);
            assert_eq!(open[0].2.range, BlockRange { from: 5, to: 9 });
            assert_eq!(open[0].2.attempts, 1);
        }

        /// A minimal `Consolidate` implementation used only to exercise the
        /// real `MessageBuffer` -> `flush_to_final_storage` ->
        /// `crosschain_messages_on_conflict` path (`message_buffer::run` is
        /// the only public entry point into it; `persistence.rs` itself is
        /// out of scope to edit per the task's hard constraints). It always
        /// produces a fresh, non-terminal `Initiated` row with
        /// `stats_processed = 0` — exactly what a freshly re-decoded replay
        /// of the same logs would rebuild, since a hot/cold entry evicted
        /// after its first final flush leaves no trace of having been
        /// finalized before.
        #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
        struct ReplayTestMessage;

        impl Consolidate for ReplayTestMessage {
            fn consolidate(&self, key: &Key) -> anyhow::Result<Option<ConsolidatedMessage>> {
                Ok(Some(ConsolidatedMessage {
                    is_final: true,
                    replace_existing: false,
                    message: interchain_indexer_entity::crosschain_messages::ActiveModel {
                        id: ActiveValue::Set(key.message_id),
                        bridge_id: ActiveValue::Set(key.bridge_id as i32),
                        status: ActiveValue::Set(
                            interchain_indexer_entity::sea_orm_active_enums::MessageStatus::Initiated,
                        ),
                        src_chain_id: ActiveValue::Set(1),
                        stats_processed: ActiveValue::Set(0),
                        ..Default::default()
                    },
                    transfers: vec![],
                    amb_confirmations: vec![],
                    amb_anomalies: vec![],
                }))
            }
        }

        /// Defect coverage: the retry pass deliberately reprocesses ranges
        /// that already succeeded (partial-success chunking, a retried
        /// chunk covering an already-resolved sub-range, etc). Replaying an
        /// interval whose message row was already persisted with a
        /// terminal status and a non-zero `stats_processed` must leave both
        /// untouched — `crosschain_messages_on_conflict` (`persistence.rs`)
        /// excludes `stats_processed` from its update set entirely, and
        /// keeps the stored `status` whenever it is already terminal.
        #[tokio::test]
        #[ignore = "needs database to run"]
        async fn replaying_an_already_persisted_interval_leaves_the_message_row_intact() {
            use interchain_indexer_entity::{
                crosschain_messages, sea_orm_active_enums::MessageStatus,
            };

            const BRIDGE_ID: i32 = 1;
            const CHAIN_ID: i64 = 1;
            const MESSAGE_ID: i64 = 424_242;
            const SEEDED_STATS_PROCESSED: i16 = 5;

            let db = init_db("range_driver_idempotent_replay_leaves_row_intact").await;
            fill_mock_interchain_database(&db).await;
            let interchain_db = InterchainDatabase::new(db.client());

            // Seed the row as if an earlier (non-replayed) pass had already
            // scanned this range to completion: terminal status, and a
            // stats counter that already ran (a sentinel value, not the
            // real projection pipeline's, so this test does not depend on
            // that pipeline's own idempotency).
            crosschain_messages::Entity::insert(crosschain_messages::ActiveModel {
                id: ActiveValue::Set(MESSAGE_ID),
                bridge_id: ActiveValue::Set(BRIDGE_ID),
                status: ActiveValue::Set(MessageStatus::Completed),
                src_chain_id: ActiveValue::Set(CHAIN_ID),
                stats_processed: ActiveValue::Set(SEEDED_STATS_PROCESSED),
                ..Default::default()
            })
            .exec(interchain_db.db.as_ref())
            .await
            .unwrap();

            // Replay: the retry path re-fetches and reprocesses the same
            // range, rebuilding the message from scratch through the normal
            // buffer/consolidation pipeline (`alter` + `run`, the same calls
            // a real `RangeProcessor::process` implementation would make).
            let buffer = MessageBuffer::<ReplayTestMessage>::new(
                interchain_db.clone(),
                MessageBufferSettings {
                    hot_ttl: Duration::from_secs(60),
                    maintenance_interval: Duration::from_secs(60),
                },
            );
            let key = Key::new(MESSAGE_ID, BRIDGE_ID as i16);
            buffer
                .alter(key, CHAIN_ID as u64, 100, |_msg| Ok(()))
                .await
                .unwrap();
            buffer.run().await.unwrap();

            let row = crosschain_messages::Entity::find_by_id((MESSAGE_ID, BRIDGE_ID))
                .one(interchain_db.db.as_ref())
                .await
                .unwrap()
                .expect("the row must still exist after replay");

            assert_eq!(
                row.status,
                MessageStatus::Completed,
                "replaying an already-persisted interval must not regress a terminal status"
            );
            assert_eq!(
                row.stats_processed, SEEDED_STATS_PROCESSED,
                "replaying an already-persisted interval must not reset stats_processed — \
                 it is excluded from crosschain_messages_on_conflict's update set"
            );
        }
    }
}
