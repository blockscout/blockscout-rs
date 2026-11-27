// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    network::Ethereum,
    primitives::{B256, U64},
    providers::{DynProvider, Provider},
    rpc::types::{Filter, Log},
};
use anyhow::{Context, Error, Result};
use bon::Builder;
use futures::{StreamExt, stream};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};

use crate::{
    InterchainDatabase,
    indexer::metrics::{LOG_STREAM_HEAD_BLOCK, LOG_STREAM_HEAD_UNAVAILABLE_TOTAL},
    log_stream::log_stream_builder::{IsSet, IsUnset, SetEnableCatchup, SetEnableRealtime, State},
};

/// Which block the realtime stream treats as its head.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HeadBlockTag {
    #[default]
    Latest,
    Safe,
    Finalized,
}

impl HeadBlockTag {
    /// The JSON-RPC block tag, also used as the `tag` metric label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Latest => "latest",
            Self::Safe => "safe",
            Self::Finalized => "finalized",
        }
    }
}

/// The two fields of a block this module needs. Deliberately not alloy's full
/// block type: resolving a tagged head must not depend on the header shape of
/// whatever fork the chain is on. Unknown fields are ignored.
#[derive(Debug, Deserialize)]
struct HeadBlock {
    number: U64,
    hash: B256,
}

/// Resolves the head block number for `tag`.
///
/// - `Latest` is `eth_blockNumber`, the call every stream made before tags
///   existed.
/// - `Safe` / `Finalized` is `eth_getBlockByNumber(<tag>, false)`. A node that
///   has no such block yet answers `null`, which is `Ok(None)`.
///
/// There is no fallback from a tagged head to `latest`: the caller waits.
pub(crate) async fn resolve_head_block(
    provider: &DynProvider<Ethereum>,
    tag: HeadBlockTag,
) -> anyhow::Result<Option<u64>> {
    if tag == HeadBlockTag::Latest {
        return Ok(Some(provider.get_block_number().await?));
    }

    let block: Option<HeadBlock> = provider
        .raw_request("eth_getBlockByNumber".into(), (tag.as_str(), false))
        .await
        .with_context(|| format!("failed to fetch the {} block", tag.as_str()))?;

    Ok(block.map(|block| {
        let number = block.number.to::<u64>();
        tracing::trace!(
            head_tag = tag.as_str(),
            number,
            hash = %block.hash,
            "resolved tagged head block"
        );
        number
    }))
}

#[derive(Builder)]
#[builder(finish_fn(name = build_config))]
pub struct LogStream {
    #[builder(start_fn)]
    provider: DynProvider<Ethereum>,
    #[builder(default = Filter::default())]
    filter: Filter,
    #[builder(default = 0)]
    genesis_block: u64,
    #[builder(default = 0)]
    realtime_cursor: u64,
    #[builder(default = 0)]
    catchup_cursor: u64,
    #[builder(default = Duration::from_secs(10))]
    poll_interval: Duration,
    #[builder(default = 100)]
    batch_size: u64,
    bridge_id: Option<i32>,
    chain_id: Option<i64>,
    /// The block the realtime stream reads up to. `Latest` (the default) keeps
    /// the `eth_blockNumber` head; a confirmed tag bounds every realtime range
    /// by that tag's block and never falls back to `latest`.
    #[builder(default)]
    head_tag: HeadBlockTag,
    /// Yield a realtime range that returned no logs as an empty `LogBatch`,
    /// so a consumer learns that the range is done (the OP sweep frontier,
    /// ADR-022). Off by default: every other stream yields non-empty batches
    /// only, as before.
    #[builder(default)]
    yield_empty_realtime: bool,
    /// Database handle used to persist the `catchup_max_cursor` checkpoint
    /// when the catchup stream finishes scanning down to `genesis_block`.
    /// Required for catchup completion to survive restarts.
    db: Option<Arc<InterchainDatabase>>,
    #[builder(setters(vis = ""))]
    enable_catchup: bool,
    #[builder(setters(vis = ""))]
    enable_realtime: bool,
}

/// How a batch reached the consumer. Metrics/logging only — no behavioural
/// branch depends on this today.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanDirection {
    Catchup,
    Realtime,
    Retry,
}

/// A batch of logs together with the block range that produced it, so a
/// consumer can attribute a downstream processing failure to a specific
/// range instead of only to the logs it happened to contain.
#[derive(Clone, Debug)]
pub struct LogBatch {
    pub from_block: u64,
    pub to_block: u64,
    pub direction: ScanDirection,
    pub logs: Vec<Log>,
}

impl<S: State> LogStreamBuilder<S> {
    /// Enable realtime mode, which will continuously poll for new logs starting
    /// from `realtime_cursor` until the stream is dropped.
    pub fn realtime(self) -> LogStreamBuilder<SetEnableRealtime<S>>
    where
        S::EnableRealtime: IsUnset,
    {
        self.enable_realtime(true)
    }

    /// Enable catchup mode, which will fetch historical logs starting from
    /// `catchup_cursor` down to `genesis_block`.
    pub fn catchup(self) -> LogStreamBuilder<SetEnableCatchup<S>>
    where
        S::EnableCatchup: IsUnset,
    {
        self.enable_catchup(true)
    }

    /// Finish the builder and immediately produce the merged log stream.
    pub fn build(self) -> anyhow::Result<stream::BoxStream<'static, LogBatch>>
    where
        S::EnableCatchup: IsSet,
        S::EnableRealtime: IsSet,
    {
        self.build_config().into_stream()
    }
}

impl LogStream {
    fn build_catchup_stream(&self) -> stream::BoxStream<'static, LogBatch> {
        let provider = self.provider.clone();
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let batch_span = batch_size.saturating_sub(1);
        let genesis_block = self.genesis_block;
        let backward_cursor = self.catchup_cursor;
        let realtime_cursor = self.realtime_cursor;
        let bridge_id = self.bridge_id;
        let chain_id = self.chain_id;
        let db = self.db.clone();

        tracing::info!(
            bridge_id,
            chain_id,
            genesis_block,
            backward_cursor,
            "Starting catchup stream"
        );

        async_stream::stream! {
            let mut to_block = backward_cursor;
            let mut observed_logs = false;
            while to_block >= genesis_block {
                let from_block = to_block.saturating_sub(batch_span).max(genesis_block);
                tracing::info!(bridge_id, chain_id, from_block, to_block, size =? (to_block - from_block + 1), "scanning CATCHUP  logs");

                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        tracing::debug!(
                            bridge_id,
                            chain_id,
                            count = logs.len(),
                            from_block,
                            to_block,
                            "fetched catchup logs"
                        );
                        if !logs.is_empty() {
                            observed_logs = true;
                            yield LogBatch {
                                from_block,
                                to_block,
                                direction: ScanDirection::Catchup,
                                logs,
                            };
                        }
                        if from_block == genesis_block {
                            break;
                        }
                        to_block = from_block.saturating_sub(1);
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            bridge_id,
                            chain_id,
                            from_block,
                            to_block,
                            "failed to fetch catchup logs batch, retrying"
                        );
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }

            let realtime_cursor_on_insert = safe_catchup_completion_realtime_cursor(
                genesis_block,
                backward_cursor,
                realtime_cursor,
                observed_logs,
            );

            persist_catchup_complete(
                db.as_deref(),
                bridge_id,
                chain_id,
                genesis_block,
                realtime_cursor_on_insert,
            )
            .await;

            tracing::info!(bridge_id, chain_id, genesis_block, "catchup complete, reached genesis block");
        }
        .map(|mut batch| {
            batch.logs.sort_by_key(|log| (log.block_number, log.log_index));
            batch.logs.reverse();
            batch
        })
        .boxed()
    }

    fn build_realtime_stream(&self) -> stream::BoxStream<'static, LogBatch> {
        let provider = self.provider.clone();
        let filter = self.filter.clone();
        let poll_interval = self.poll_interval;
        let batch_size = self.batch_size;
        let batch_span = batch_size.saturating_sub(1);
        let realtime_cursor = self.realtime_cursor;
        let bridge_id = self.bridge_id;
        let chain_id = self.chain_id;
        let head_tag = self.head_tag;
        let yield_empty_realtime = self.yield_empty_realtime;
        // Only a tagged head is exported: the metric surface of every stream
        // that reads to `latest` stays exactly what it was.
        let head_metric_labels = (head_tag != HeadBlockTag::Latest).then(|| {
            [
                bridge_id.map(|id| id.to_string()).unwrap_or_default(),
                chain_id.map(|id| id.to_string()).unwrap_or_default(),
                head_tag.as_str().to_string(),
            ]
        });

        tracing::info!(
            bridge_id,
            chain_id,
            realtime_cursor,
            head_tag = head_tag.as_str(),
            "Starting realtime stream"
        );

        async_stream::stream! {
            let mut from_block = realtime_cursor;
            loop {
                // `Err` and "no such block yet" both mean: wait one interval and
                // ask again. A tagged head never falls back to `latest`.
                let head = match resolve_head_block(&provider, head_tag).await {
                    Ok(Some(head)) => {
                        if let Some([bridge, chain, tag]) = &head_metric_labels {
                            LOG_STREAM_HEAD_BLOCK
                                .with_label_values(&[bridge.as_str(), chain.as_str(), tag.as_str()])
                                .set(head as f64);
                        }
                        Some(head)
                    }
                    Ok(None) => {
                        tracing::debug!(
                            bridge_id,
                            chain_id,
                            head_tag = head_tag.as_str(),
                            "head block is not available yet"
                        );
                        None
                    }
                    Err(e) => {
                        tracing::error!(
                            err =? e,
                            bridge_id,
                            chain_id,
                            head_tag = head_tag.as_str(),
                            "failed to resolve head block"
                        );
                        None
                    }
                };
                if head.is_none()
                    && let Some([bridge, chain, tag]) = &head_metric_labels
                {
                    LOG_STREAM_HEAD_UNAVAILABLE_TOTAL
                        .with_label_values(&[bridge.as_str(), chain.as_str(), tag.as_str()])
                        .inc();
                }

                let to_block = head
                    .filter(|head| from_block <= *head)
                    .map(|head| from_block.saturating_add(batch_span).min(head));

                let Some(to_block) = to_block else {
                    tracing::debug!(bridge_id, chain_id, from_block, head_tag = head_tag.as_str(), "waiting for new blocks");
                    tokio::time::sleep(poll_interval).await;
                    continue;
                };

                tracing::info!(bridge_id, chain_id, from_block, to_block, size =? (to_block - from_block + 1), "scanning REALTIME logs");
                match fetch_logs(provider.clone(), &filter, from_block, to_block).await {
                    Ok(logs) => {
                        if !logs.is_empty() || yield_empty_realtime {
                            tracing::debug!(
                                bridge_id,
                                chain_id,
                                count = logs.len(),
                                from_block,
                                to_block,
                                batch_size,
                                "fetched realtime logs"
                            );
                            yield LogBatch {
                                from_block,
                                to_block,
                                direction: ScanDirection::Realtime,
                                logs,
                            };
                        }
                        from_block = to_block + 1;
                    }
                    Err(e) => {
                        if is_get_logs_error_silent(&e) {
                            tracing::debug!(
                                err =? e,
                                bridge_id,
                                chain_id,
                                from_block,
                                to_block,
                                "realtime logs are not available at the reported latest block yet, retrying"
                            );
                        } else {
                            tracing::error!(
                                err =? e,
                                bridge_id,
                                chain_id,
                                from_block,
                                to_block,
                                "failed to fetch realtime logs"
                            );
                        }
                        tokio::time::sleep(poll_interval).await;
                        continue;
                    }
                }

                tokio::time::sleep(poll_interval).await;
            }
        }
        .map(|mut batch| {
            batch.logs.sort_by_key(|log| (log.block_number, log.log_index));
            batch
        })
        .boxed()
    }

    pub fn into_stream(self) -> Result<stream::BoxStream<'static, LogBatch>> {
        if self.realtime_cursor < self.catchup_cursor {
            Err(anyhow::anyhow!(
                "realtime_cursor ({}) must be >= catchup_cursor ({})",
                self.realtime_cursor,
                self.catchup_cursor
            ))?;
        };

        let mut combined = stream::empty().boxed();

        if self.enable_catchup {
            combined = stream::select(combined, self.build_catchup_stream()).boxed();
        }

        if self.enable_realtime {
            combined = stream::select(combined, self.build_realtime_stream()).boxed();
        }

        Ok(Box::pin(combined))
    }
}

/// TODO: This fetcher should also be able to continue it's operation if it hits
/// the rpc error (for instance if some eth_getLogs call wasn't successful), it
/// should also be able to dynamically configure block range that is included
/// into this eth_getLogs. for instance, if it fetches block range [0; 100] and
/// block #14 is bad, it should still be able to fetch all blocks that are okay,
/// so [0;13] and [14; 100] should be fetched though and #14 should be marked as
/// a bad block.
pub(crate) async fn fetch_logs(
    provider: DynProvider<Ethereum>,
    filter: &Filter,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<Log>> {
    let filter = filter.clone().from_block(from_block).to_block(to_block);
    let logs = provider.get_logs(&filter).await?;
    Ok(logs)
}

pub(crate) fn is_get_logs_error_silent(err: &Error) -> bool {
    err.chain().any(|cause| {
        cause
            .to_string()
            .contains("from block is greater than latest block")
    })
}

/// Persist that catchup has scanned down to `genesis_block`. Without this,
/// `catchup_max_cursor` would remain at the last observed message and a
/// restart would re-walk the empty range below it on every boot.
async fn persist_catchup_complete(
    db: Option<&InterchainDatabase>,
    bridge_id: Option<i32>,
    chain_id: Option<i64>,
    genesis_block: u64,
    realtime_cursor_on_insert: Option<u64>,
) {
    let (Some(db), Some(bridge_id), Some(chain_id)) = (db, bridge_id, chain_id) else {
        tracing::warn!(
            bridge_id,
            chain_id,
            "skipping catchup checkpoint persistence: db/bridge_id/chain_id missing"
        );
        return;
    };

    if let Err(err) = db
        .mark_catchup_complete(
            bridge_id as u64,
            chain_id as u64,
            genesis_block.saturating_sub(1),
            realtime_cursor_on_insert,
        )
        .await
    {
        tracing::error!(
            err = ?err,
            bridge_id,
            chain_id,
            genesis_block,
            realtime_cursor_on_insert,
            "failed to persist catchup completion checkpoint"
        );
    }
}

fn safe_catchup_completion_realtime_cursor(
    genesis_block: u64,
    catchup_cursor: u64,
    realtime_cursor: u64,
    observed_catchup_logs: bool,
) -> Option<u64> {
    if observed_catchup_logs || realtime_cursor < genesis_block {
        return None;
    }

    // `realtime_cursor` is an inclusive realtime start block, not an
    // already-processed cursor. It is safe to persist on insert only when the
    // completed catchup pass covered every configured block below it and found
    // no logs; otherwise a restart should rescan instead of skipping forward.
    (catchup_cursor.saturating_add(1) >= realtime_cursor).then_some(realtime_cursor)
}

/// A mocked JSON-RPC transport for tests of the head tag: it records every
/// request and answers the three methods a `LogStream` can issue.
#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        collections::VecDeque,
        future,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU64, Ordering},
        },
        task::{Context as TaskContext, Poll},
    };

    use alloy::{
        network::Ethereum,
        primitives::B256,
        providers::{DynProvider, Provider, ProviderBuilder},
        rpc::{client::RpcClient, types::Log},
        transports::{TransportError, TransportErrorKind, TransportFut},
    };
    use alloy_json_rpc::{Id, RequestPacket, Response, ResponsePacket, ResponsePayload};
    use serde_json::{Value, json};
    use tower::Service;

    /// One scripted answer to `eth_getBlockByNumber(<tag>, false)`.
    #[derive(Clone, Copy, Debug)]
    pub(crate) enum TaggedHead {
        Block(u64),
        /// The node has no such block yet: a JSON `null` result.
        Null,
        /// A transport error.
        Error,
    }

    /// One scripted answer to `eth_getLogs`.
    #[derive(Clone, Debug)]
    pub(crate) enum ScriptedLogs {
        Logs(Vec<Log>),
        Error,
        /// Answers like `Logs`, but only once the test opens the gate: the mock
        /// signals `entered`, then waits for `release`.
        Gated(Vec<Log>, LogsGate),
    }

    /// Holds one scripted `eth_getLogs` answer until the test releases it.
    #[derive(Clone, Debug, Default)]
    pub(crate) struct LogsGate {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    impl LogsGate {
        /// Resolves once the mock has received the gated request.
        pub(crate) async fn entered(&self) {
            self.entered.notified().await;
        }

        /// Lets the gated answer go.
        pub(crate) fn release(&self) {
            self.release.notify_one();
        }
    }

    /// `eth_blockNumber` always answers `latest`. `eth_getBlockByNumber`
    /// answers from the scripted queue, repeating the last entry once the
    /// queue is down to one. `eth_getLogs` answers from the scripted queue
    /// (`with_logs`), front first; once it is empty, one dummy log per call,
    /// because a `LogStream` yields only non-empty batches unless it is built with
    /// `yield_empty_realtime`.
    #[derive(Clone)]
    pub(crate) struct HeadMockService {
        latest: Arc<AtomicU64>,
        tagged_heads: Arc<Mutex<VecDeque<TaggedHead>>>,
        logs_script: Arc<Mutex<VecDeque<ScriptedLogs>>>,
        calls: Arc<Mutex<Vec<(String, Value)>>>,
    }

    impl HeadMockService {
        pub(crate) fn new(latest: u64, tagged_heads: Vec<TaggedHead>) -> Self {
            Self {
                latest: Arc::new(AtomicU64::new(latest)),
                tagged_heads: Arc::new(Mutex::new(tagged_heads.into())),
                logs_script: Arc::new(Mutex::new(VecDeque::new())),
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        /// `eth_getLogs` answers from this queue, front first; once it is
        /// empty, the default single dummy log at `fromBlock`.
        pub(crate) fn with_logs(self, script: Vec<ScriptedLogs>) -> Self {
            *self.logs_script.lock().expect("lock poisoned") = script.into();
            self
        }

        pub(crate) fn set_latest(&self, latest: u64) {
            self.latest.store(latest, Ordering::SeqCst);
        }

        /// Every `(method, params)` received so far, in order.
        pub(crate) fn calls(&self) -> Vec<(String, Value)> {
            self.calls.lock().expect("lock poisoned").clone()
        }

        pub(crate) fn provider(&self) -> DynProvider<Ethereum> {
            let client = RpcClient::builder().transport(self.clone(), false);
            ProviderBuilder::new().connect_client(client).erased()
        }

        fn next_tagged_head(&self) -> Option<TaggedHead> {
            let mut heads = self.tagged_heads.lock().expect("lock poisoned");
            if heads.len() > 1 {
                heads.pop_front()
            } else {
                heads.front().copied()
            }
        }
    }

    impl Service<RequestPacket> for HeadMockService {
        type Response = ResponsePacket;
        type Error = TransportError;
        type Future = TransportFut<'static>;

        fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: RequestPacket) -> Self::Future {
            let mut gated: Option<(Id, Vec<Log>, LogsGate)> = None;
            let result = match req.as_single() {
                Some(request) => {
                    let method = request.method().to_string();
                    let params: Value = request
                        .params()
                        .and_then(|raw| serde_json::from_str(raw.get()).ok())
                        .unwrap_or(Value::Null);
                    self.calls
                        .lock()
                        .expect("lock poisoned")
                        .push((method.clone(), params.clone()));
                    let id = request.id().clone();
                    match method.as_str() {
                        "eth_blockNumber" => Ok(json_response(
                            id,
                            &json!(format!("{:#x}", self.latest.load(Ordering::SeqCst))),
                        )),
                        "eth_getBlockByNumber" => match self.next_tagged_head() {
                            Some(TaggedHead::Block(number)) => Ok(json_response(
                                id,
                                // More fields than the minimal decode reads, to
                                // show that the rest of the block is ignored.
                                &json!({
                                    "number": format!("{number:#x}"),
                                    "hash": B256::with_last_byte(1),
                                    "timestamp": "0x1",
                                    "someFutureHeaderField": {"nested": true},
                                }),
                            )),
                            Some(TaggedHead::Null) => Ok(json_response(id, &Value::Null)),
                            Some(TaggedHead::Error) | None => Err(TransportErrorKind::custom_str(
                                "HeadMockService: scripted head error",
                            )),
                        },
                        "eth_getLogs" => {
                            let scripted =
                                self.logs_script.lock().expect("lock poisoned").pop_front();
                            match scripted {
                                Some(ScriptedLogs::Logs(logs)) => Ok(json_response(id, &logs)),
                                Some(ScriptedLogs::Error) => Err(TransportErrorKind::custom_str(
                                    "HeadMockService: scripted logs error",
                                )),
                                Some(ScriptedLogs::Gated(logs, gate)) => {
                                    gated = Some((id, logs, gate));
                                    Err(TransportErrorKind::custom_str("unreachable"))
                                }
                                None => {
                                    let log: Log = Log {
                                        block_number: requested_range(&params)
                                            .map(|(from, _)| from),
                                        log_index: Some(0),
                                        ..Default::default()
                                    };
                                    Ok(json_response(id, &vec![log]))
                                }
                            }
                        }
                        other => Err(TransportErrorKind::custom_str(&format!(
                            "HeadMockService: unexpected method {other}"
                        ))),
                    }
                }
                None => Err(TransportErrorKind::custom_str(
                    "HeadMockService: expected a single request",
                )),
            };
            if let Some((id, logs, gate)) = gated {
                return Box::pin(async move {
                    gate.entered.notify_one();
                    gate.release.notified().await;
                    Ok(json_response(id, &logs))
                });
            }
            Box::pin(future::ready(result))
        }
    }

    fn json_response<T: serde::Serialize>(id: Id, value: &T) -> ResponsePacket {
        let payload = serde_json::value::to_raw_value(value).expect("serialize mock response");
        ResponsePacket::Single(Response {
            id,
            payload: ResponsePayload::Success(payload),
        })
    }

    /// `(fromBlock, toBlock)` of an `eth_getLogs` request.
    pub(crate) fn requested_range(params: &Value) -> Option<(u64, u64)> {
        let block = |name: &str| {
            let hex = params.get(0)?.get(name)?.as_str()?;
            u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
        };
        Some((block("fromBlock")?, block("toBlock")?))
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::{
        test_support::{HeadMockService, ScriptedLogs, TaggedHead, requested_range},
        *,
    };

    /// A stream that only does realtime work: catch-up starts below its own
    /// floor and finishes at once, without a database. The poll interval is a
    /// few milliseconds of real time; the mock answers at once, so the order
    /// of requests does not depend on it.
    fn realtime_only_stream(
        mock: &HeadMockService,
        cursor: u64,
        head_tag: Option<HeadBlockTag>,
    ) -> stream::BoxStream<'static, LogBatch> {
        realtime_only_stream_with(mock, cursor, head_tag, false)
    }

    fn realtime_only_stream_with(
        mock: &HeadMockService,
        cursor: u64,
        head_tag: Option<HeadBlockTag>,
        yield_empty_realtime: bool,
    ) -> stream::BoxStream<'static, LogBatch> {
        let builder = LogStream::builder(mock.provider())
            .genesis_block(cursor)
            .realtime_cursor(cursor)
            .catchup_cursor(cursor - 1)
            .poll_interval(Duration::from_millis(5))
            .batch_size(100)
            .yield_empty_realtime(yield_empty_realtime);
        match head_tag {
            Some(head_tag) => builder.head_tag(head_tag).catchup().realtime().build(),
            None => builder.catchup().realtime().build(),
        }
        .expect("stream builds")
    }

    fn methods(mock: &HeadMockService) -> Vec<String> {
        mock.calls().into_iter().map(|(method, _)| method).collect()
    }

    fn log_ranges(mock: &HeadMockService) -> Vec<(u64, u64)> {
        mock.calls()
            .iter()
            .filter(|(method, _)| method == "eth_getLogs")
            .map(|(_, params)| requested_range(params).expect("eth_getLogs carries a block range"))
            .collect()
    }

    #[tokio::test]
    async fn test_realtime_stream_finalized_tag_never_reads_above_head() {
        let heads = [105, 103, 110];
        let mock = HeadMockService::new(
            1_000,
            heads.iter().copied().map(TaggedHead::Block).collect(),
        );
        let mut stream = realtime_only_stream(&mock, 100, Some(HeadBlockTag::Finalized));

        let first = stream.next().await.expect("first batch");
        let second = stream.next().await.expect("second batch");

        assert_eq!((first.from_block, first.to_block), (100, 105));
        assert_eq!((second.from_block, second.to_block), (106, 110));
        assert_eq!(log_ranges(&mock), vec![(100, 105), (106, 110)]);
        // Nothing is requested while the head (103) is behind the cursor (106).
        assert_eq!(
            methods(&mock),
            vec![
                "eth_getBlockByNumber",
                "eth_getLogs",
                "eth_getBlockByNumber",
                "eth_getBlockByNumber",
                "eth_getLogs",
            ]
        );

        // Every range ends at or below the head it was derived from, i.e. the
        // last head answered before that `eth_getLogs`.
        let mut answered_heads = heads.iter();
        let mut current_head = None;
        for (method, params) in mock.calls() {
            match method.as_str() {
                "eth_getBlockByNumber" => {
                    assert_eq!(params, serde_json::json!(["finalized", false]));
                    current_head = answered_heads.next().copied();
                }
                "eth_getLogs" => {
                    let (_, to_block) = requested_range(&params).expect("block range");
                    let head = current_head.expect("a head precedes every eth_getLogs");
                    assert!(
                        to_block <= head,
                        "range end {to_block} is above head {head}"
                    );
                }
                other => panic!("unexpected RPC method {other}"),
            }
        }
    }

    #[tokio::test]
    async fn test_realtime_stream_tagged_head_unavailable_waits_and_recovers() {
        let mock = HeadMockService::new(
            1_000,
            vec![TaggedHead::Null, TaggedHead::Error, TaggedHead::Block(105)],
        );
        let mut stream = realtime_only_stream(&mock, 100, Some(HeadBlockTag::Safe));

        let batch = stream.next().await.expect("batch after the head recovers");

        assert_eq!((batch.from_block, batch.to_block), (100, 105));
        // No `eth_getLogs` before the third answer, and never `eth_blockNumber`:
        // an unavailable tagged head does not fall back to `latest`.
        assert_eq!(
            methods(&mock),
            vec![
                "eth_getBlockByNumber",
                "eth_getBlockByNumber",
                "eth_getBlockByNumber",
                "eth_getLogs",
            ]
        );
        assert!(
            mock.calls()
                .iter()
                .filter(|(method, _)| method == "eth_getBlockByNumber")
                .all(|(_, params)| *params == serde_json::json!(["safe", false]))
        );
        assert_eq!(log_ranges(&mock), vec![(100, 105)]);
    }

    #[tokio::test]
    async fn test_realtime_stream_latest_tag_uses_eth_block_number_only() {
        let mock = HeadMockService::new(105, vec![TaggedHead::Block(50)]);
        let mut stream = realtime_only_stream(&mock, 100, None);

        let batch = stream.next().await.expect("batch");

        assert_eq!((batch.from_block, batch.to_block), (100, 105));
        assert_eq!(methods(&mock), vec!["eth_blockNumber", "eth_getLogs"]);
    }

    #[tokio::test]
    async fn test_realtime_stream_yields_empty_ranges_when_asked() {
        let mock =
            HeadMockService::new(1_000, vec![TaggedHead::Block(105), TaggedHead::Block(110)])
                .with_logs(vec![ScriptedLogs::Logs(vec![]), ScriptedLogs::Logs(vec![])]);
        let mut stream = realtime_only_stream_with(&mock, 100, Some(HeadBlockTag::Finalized), true);

        let first = stream.next().await.expect("first batch");
        let second = stream.next().await.expect("second batch");

        assert_eq!((first.from_block, first.to_block), (100, 105));
        assert_eq!((second.from_block, second.to_block), (106, 110));
        assert!(first.logs.is_empty());
        assert!(second.logs.is_empty());
    }

    #[tokio::test]
    async fn test_realtime_stream_skips_empty_ranges_by_default() {
        let mock =
            HeadMockService::new(1_000, vec![TaggedHead::Block(105), TaggedHead::Block(110)])
                .with_logs(vec![ScriptedLogs::Logs(vec![]), ScriptedLogs::Logs(vec![])]);
        let mut stream = realtime_only_stream(&mock, 100, Some(HeadBlockTag::Finalized));

        let next = tokio::time::timeout(Duration::from_millis(100), stream.next()).await;

        assert!(next.is_err(), "an empty range must not be yielded");
        assert!(
            methods(&mock).iter().any(|method| method == "eth_getLogs"),
            "eth_getLogs was still called"
        );
    }

    #[test]
    fn recognizes_realtime_tip_ahead_error() {
        let err = anyhow::anyhow!(
            "server returned an error response: error code -32602: invalid params, data: \"from block is greater than latest block\""
        );

        assert!(is_get_logs_error_silent(&err));
    }

    #[test]
    fn recognizes_realtime_tip_ahead_error_in_source_chain() {
        let err = Err::<(), _>(anyhow::anyhow!(
            "server returned an error response: error code -32602: invalid params, data: \"from block is greater than latest block\""
        ))
        .context("failed to fetch logs")
        .unwrap_err();

        assert!(is_get_logs_error_silent(&err));
    }

    #[test]
    fn ignores_other_realtime_errors() {
        let err = anyhow::anyhow!(
            "server returned an error response: error code -32005: query returned more than 10000 results"
        );

        assert!(!is_get_logs_error_silent(&err));
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_allows_empty_contiguous_range() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 99, 100, false);

        assert_eq!(cursor, Some(100));
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_observed_logs() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 99, 100, true);

        assert_eq!(cursor, None);
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_gap_before_realtime() {
        let cursor = safe_catchup_completion_realtime_cursor(10, 90, 100, false);

        assert_eq!(cursor, None);
    }

    #[test]
    fn safe_catchup_completion_realtime_cursor_rejects_invalid_insert_state() {
        let cursor = safe_catchup_completion_realtime_cursor(100, 99, 50, false);

        assert_eq!(cursor, None);
    }
}
