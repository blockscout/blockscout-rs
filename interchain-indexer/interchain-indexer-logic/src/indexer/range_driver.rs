// SPDX-License-Identifier: LicenseRef-Blockscout

use std::sync::Arc;

use alloy::{network::Ethereum, providers::DynProvider, rpc::types::Filter};
use anyhow::bail;
use futures::{Stream, StreamExt};
use tonic::async_trait;

use crate::{
    indexer::{
        failure_ledger::{
            FailureLedger,
            interval::{BlockRange, FailedInterval},
            policy,
            settings::FailureRetrySettings,
        },
        metrics,
    },
    log_stream::{LogBatch, ScanDirection, fetch_logs},
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

/// `#[async_trait]` rather than native AFIT: the provided `retry_pending`
/// default awaits other trait methods and the whole driver loop runs inside
/// a `tokio::spawn`ed task, so `Send` futures are required — which RPITIT
/// cannot express for a provided body. (`.memory-bank/rules/async-patterns.md`
/// requires this justification.)
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

    /// Provided default: re-fetch each due chunk via `log_filter`/`provider`,
    /// call `process`, then report the outcome through the ledger. EVM-shaped
    /// indexers need no override.
    ///
    /// Chunking is mandatory, not an optimisation: a recorded interval can be
    /// far wider than `batch_size` because `record` merges adjacent ranges on
    /// purpose. Each chunk is resolved or re-recorded on its own, so partial
    /// progress survives a failure partway through a wide interval. Bounded
    /// by `max_chunks` across every due interval combined, so a large hole
    /// set cannot starve the realtime scan sharing this task.
    async fn retry_pending(
        &self,
        ledger: &FailureLedger,
        due: &[(i64, FailedInterval)],
        max_chunks: usize,
    ) {
        let bridge_id = self.bridge_id();
        let batch_size = self.batch_size().max(1);
        let mut chunks_attempted = 0usize;

        for (chain_id, interval) in due {
            if chunks_attempted >= max_chunks {
                break;
            }

            let chain_id = *chain_id;

            let Some(provider) = self.provider(chain_id) else {
                tracing::error!(
                    bridge_id,
                    chain_id,
                    "no provider configured for chain during retry pass"
                );
                continue;
            };

            let filter = match self.log_filter(chain_id) {
                Ok(filter) => filter,
                Err(err) => {
                    tracing::error!(err = ?err, bridge_id, chain_id, "failed to build log filter during retry pass");
                    continue;
                }
            };

            let mut from = interval.range.from;
            let to = interval.range.to;

            while from <= to {
                if chunks_attempted >= max_chunks {
                    break;
                }

                let chunk_to = from.saturating_add(batch_size.saturating_sub(1)).min(to);
                let chunk = BlockRange { from, to: chunk_to };
                chunks_attempted += 1;

                match fetch_logs(provider.clone(), &filter, chunk.from, chunk.to).await {
                    Ok(mut logs) => {
                        // Parity with the forward path's ascending sort.
                        logs.sort_by_key(|log| (log.block_number, log.log_index));
                        let batch = LogBatch {
                            from_block: chunk.from,
                            to_block: chunk.to,
                            direction: ScanDirection::Retry,
                            logs,
                        };

                        match self.process(chain_id, &batch).await {
                            Ok(()) => {
                                // A retried chunk that returns zero logs must
                                // still be resolved: the forward path never
                                // yields empty ranges, so this case exists
                                // only here.
                                if let Err(err) =
                                    ledger.resolve(bridge_id, chain_id, &[chunk]).await
                                {
                                    tracing::error!(
                                        err = ?err,
                                        bridge_id,
                                        chain_id,
                                        chunk_from = chunk.from,
                                        chunk_to = chunk.to,
                                        direction = ?batch.direction,
                                        "failed to resolve a successfully retried chunk"
                                    );
                                }
                            }
                            Err(batch_err) => {
                                let ranges = attributed_ranges(&batch_err, chunk);
                                let reason = truncate_reason(&format!("{:#}", batch_err.error));
                                let ranges_with_reason = with_reason(ranges, reason);
                                if let Err(err) = ledger
                                    .record(bridge_id, chain_id, &ranges_with_reason)
                                    .await
                                {
                                    tracing::error!(
                                        err = ?err,
                                        bridge_id,
                                        chain_id,
                                        chunk_from = chunk.from,
                                        chunk_to = chunk.to,
                                        direction = ?batch.direction,
                                        "failed to re-record a still-failing retried chunk"
                                    );
                                }
                            }
                        }
                    }
                    Err(err) => {
                        // An eth_getLogs failure records the chunk as
                        // still-failed and moves on — this is not an
                        // escalation, the range was never re-scanned.
                        let reason = truncate_reason(&format!("{:#}", err));
                        if let Err(record_err) =
                            ledger.record(bridge_id, chain_id, &[(chunk, reason)]).await
                        {
                            tracing::error!(
                                err = ?record_err,
                                bridge_id,
                                chain_id,
                                chunk_from = chunk.from,
                                chunk_to = chunk.to,
                                "failed to re-record a retry-fetch failure"
                            );
                        }
                    }
                }

                if chunk_to == to {
                    break;
                }
                from = chunk_to.saturating_add(1);
            }
        }
    }
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

    /// Consumes the merged per-chain stream until it ends (`Ok(())`), or
    /// returns `Err` on the escalation path only.
    pub async fn run(
        self,
        mut stream: impl Stream<Item = (i64, LogBatch)> + Unpin + Send,
    ) -> anyhow::Result<()> {
        let bridge_id = self.processor.bridge_id();
        let pairs: Vec<(i32, i64)> = self
            .processor
            .chain_ids()
            .into_iter()
            .map(|chain_id| (bridge_id, chain_id))
            .collect();

        self.ledger.initialize(&pairs).await?;

        let mut retry_tick = tokio::time::interval(self.settings.scan_interval);
        // The default (Burst) would fire a run of catch-up ticks immediately
        // after a long retry pass.
        retry_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                item = stream.next() => {
                    match item {
                        None => break,
                        Some((chain_id, batch)) => {
                            self.handle_batch(bridge_id, chain_id, batch).await?;
                        }
                    }
                }
                _ = retry_tick.tick() => {
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
                        self.run_retry_tick(bridge_id, &pairs).await;
                    }
                }
            }
        }

        Ok(())
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
                        err = ?err,
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
                let ranges = attributed_ranges(&batch_err, range);
                let reason = truncate_reason(&format!("{:#}", batch_err.error));
                let ranges_with_reason = with_reason(ranges, reason);

                match self
                    .ledger
                    .record(bridge_id, chain_id, &ranges_with_reason)
                    .await
                {
                    Ok(()) => {
                        tracing::error!(
                            err = ?batch_err.error,
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
                                .with_label_values(&[&bridge_id.to_string()])
                                .inc();
                            // With no cursor barrier, `record()` is the last
                            // point where data can be permanently lost.
                            // Stopping here is provably safe: realtime is
                            // monotone forward per chain, so once the driver
                            // stops consuming, no buffer entry above the
                            // failed interval can appear and the cursor
                            // cannot be derived past it.
                            bail!(
                                "unable to record indexer failure for bridge {bridge_id} chain {chain_id} \
                                 range [{}, {}] after {} attempt(s) (processing error: {:#}): {final_err:#}",
                                range.from,
                                range.to,
                                self.settings.record_retry_attempts,
                                batch_err.error,
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

    async fn run_retry_tick(&self, bridge_id: i32, pairs: &[(i32, i64)]) {
        let open = match self.ledger.open(pairs).await {
            Ok(open) => open,
            Err(err) => {
                tracing::error!(err = ?err, bridge_id, "failed to query open indexer failures for retry pass");
                return;
            }
        };

        let now = chrono::Utc::now().naive_utc();
        let due: Vec<(i64, FailedInterval)> = open
            .into_iter()
            .filter(|(_, _, interval)| {
                policy::is_due(
                    interval,
                    now,
                    self.settings.backoff_base,
                    self.settings.backoff_cap,
                )
            })
            .map(|(_, chain_id, interval)| (chain_id, interval))
            .collect();

        self.processor
            .retry_pending(&self.ledger, &due, self.settings.max_chunks_per_pass)
            .await;
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
                let result = match action {
                    Some(MockLogsAction::Logs(logs)) => Ok(build_logs_response(&req, &logs)),
                    Some(MockLogsAction::Error(msg)) => Err(TransportErrorKind::custom_str(msg)),
                    None => Err(TransportErrorKind::custom_str("no mock action queued")),
                };
                Box::pin(future::ready(result))
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
        /// (recording, replay, escalation) comes from the trait's provided
        /// `retry_pending` default and `RangeDriver`.
        struct TestRangeProcessor {
            bridge_id: i32,
            chain_ids: Vec<i64>,
            batch_size: u64,
            providers: HashMap<i64, DynProvider<Ethereum>>,
            fail_exact: Arc<Mutex<HashSet<(i64, u64, u64)>>>,
            process_calls: Arc<AtomicUsize>,
        }

        impl TestRangeProcessor {
            fn new(bridge_id: i32, chain_ids: Vec<i64>, batch_size: u64) -> Self {
                Self {
                    bridge_id,
                    chain_ids,
                    batch_size,
                    providers: HashMap::new(),
                    fail_exact: Arc::new(Mutex::new(HashSet::new())),
                    process_calls: Arc::new(AtomicUsize::new(0)),
                }
            }

            fn with_provider(mut self, chain_id: i64, provider: DynProvider<Ethereum>) -> Self {
                self.providers.insert(chain_id, provider);
                self
            }

            fn fail_range(&self, chain_id: i64, from: u64, to: u64) {
                self.fail_exact.lock().insert((chain_id, from, to));
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
                self.providers.get(&chain_id).cloned()
            }

            fn log_filter(&self, _chain_id: i64) -> anyhow::Result<Filter> {
                Ok(Filter::default())
            }

            fn batch_size(&self) -> u64 {
                self.batch_size
            }

            async fn process(&self, chain_id: i64, batch: &LogBatch) -> Result<(), BatchError> {
                self.process_calls.fetch_add(1, Ordering::SeqCst);
                let should_fail =
                    self.fail_exact
                        .lock()
                        .contains(&(chain_id, batch.from_block, batch.to_block));
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
                .run(stream)
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
                .run(stream)
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
                .run(stream)
                .await;

            assert!(result.is_err(), "an unrecordable failure must escalate");
            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                1,
                "the driver must not request the next batch after escalating"
            );
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
            // Normally done by `RangeDriver::run` before the loop starts;
            // calling `retry_pending` directly here (bypassing `run`) must
            // warm the cache itself, or `resolve` below would silently
            // short-circuit on a cache that (wrongly) says no holes exist.
            ledger.initialize(&[(1, 1)]).await.unwrap();
            let due: Vec<(i64, FailedInterval)> = ledger
                .open(&[(1, 1)])
                .await
                .unwrap()
                .into_iter()
                .map(|(_, chain_id, interval)| (chain_id, interval))
                .collect();
            assert_eq!(due.len(), 1);

            let mock_service = MockLogsService::new();
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));

            processor.retry_pending(&ledger, &due, 16).await;

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
            let due: Vec<(i64, FailedInterval)> = ledger
                .open(&[(1, 1)])
                .await
                .unwrap()
                .into_iter()
                .map(|(_, chain_id, interval)| (chain_id, interval))
                .collect();

            let mock_service = MockLogsService::new();
            // Three 1000-block chunks; all fetch successfully.
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 1000)
                .with_provider(1, mock_provider(mock_service));
            // The middle chunk fails at the `process()` level.
            processor.fail_range(1, 1000, 1999);

            processor.retry_pending(&ledger, &due, 16).await;

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
        /// cannot starve the realtime scan sharing this task.
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
            let due: Vec<(i64, FailedInterval)> = ledger
                .open(&[(1, 1)])
                .await
                .unwrap()
                .into_iter()
                .map(|(_, chain_id, interval)| (chain_id, interval))
                .collect();

            let mock_service = MockLogsService::new();
            mock_service.push_logs(vec![]);
            mock_service.push_logs(vec![]);

            let processor = TestRangeProcessor::new(1, vec![1], 1000)
                .with_provider(1, mock_provider(mock_service));
            let process_calls = processor.process_calls.clone();

            processor.retry_pending(&ledger, &due, 2).await;

            assert_eq!(
                process_calls.load(Ordering::SeqCst),
                2,
                "exactly max_chunks_per_pass chunks must be attempted"
            );

            let open = ledger.open(&[(1, 1)]).await.unwrap();
            assert_eq!(open.len(), 1);
            // The first two 1000-block chunks resolved; the rest remains.
            assert_eq!(
                open[0].2.range,
                BlockRange {
                    from: 2000,
                    to: 4999
                }
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
            let due: Vec<(i64, FailedInterval)> = ledger
                .open(&[(1, 1)])
                .await
                .unwrap()
                .into_iter()
                .map(|(_, chain_id, interval)| (chain_id, interval))
                .collect();

            let mock_service = MockLogsService::new();
            mock_service.push_error("rpc unavailable");

            let processor = TestRangeProcessor::new(1, vec![1], 100)
                .with_provider(1, mock_provider(mock_service));

            processor.retry_pending(&ledger, &due, 16).await;

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
