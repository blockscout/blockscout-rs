use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use alloy::{
    network::Ethereum,
    primitives::{Address, B256},
    providers::DynProvider,
    rpc::types::{Filter, Log},
};
use anyhow::{Context, Result, anyhow, ensure};
use dashmap::DashMap;
use serde_json::Value;
use tokio::task::JoinHandle;
use tonic::async_trait;

use crate::{
    CrosschainIndexer, CrosschainIndexerState, CrosschainIndexerStatus, InterchainDatabase,
    MessageBufferSettings, StatsService,
    indexer::{
        failure_ledger::{BlockRange, FailureLedger},
        range_driver::{BatchError, RangeDriver, RangeProcessor},
    },
    log_stream::LogBatch,
    message_buffer::{Key, MessageBuffer},
    secret::redact_urls,
};

use super::{
    abi::AbiRegistry,
    events::{self, EventContext, PendingMessageHashEvents},
    settings::AmbIndexerSettings,
    types::Message,
};

/// One configured deployment of an AMB contract, valid from `started_at_block`
/// until the next version of the same address begins.
#[derive(Clone, Debug)]
pub struct AmbContractConfig {
    pub address: Address,
    pub version: i16,
    pub started_at_block: u64,
    pub abi: Option<Value>,
}

/// One chain this bridge indexes, with every configured version of each kind.
///
/// Lists rather than single fields: an implementation upgrade is expressed as a
/// new config entry with a higher `version` and the block it takes effect from
/// (`bridge_contracts` keys on `(bridge_id, chain_id, address, version)`), and
/// a proxy is normally upgraded behind the *same* address. Collapsing each kind
/// to one entry silently indexed whichever the config happened to list first
/// and dropped the rest.
#[derive(Clone)]
pub struct AmbChainConfig {
    pub chain_id: i64,
    pub provider: DynProvider<Ethereum>,
    /// The block the scan starts from: the lowest `started_at_block` among
    /// `amb_proxies`.
    pub start_block: u64,
    /// At least one; AMB cannot index a chain without a proxy.
    pub amb_proxies: Vec<AmbContractConfig>,
    /// May be empty — messages are then indexed without token transfers.
    pub mediators: Vec<AmbContractConfig>,
}

pub struct AmbIndexer {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    chains: Vec<AmbChainConfig>,
    abi_registry: Arc<AbiRegistry>,
    message_hash_lookup: Arc<DashMap<B256, Key>>,
    pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>>,
    settings: AmbIndexerSettings,
    buffer: Arc<MessageBuffer<Message>>,
    buffer_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    is_running: Arc<AtomicBool>,
    indexing_handle: Arc<parking_lot::RwLock<Option<JoinHandle<()>>>>,
    state: Arc<parking_lot::RwLock<CrosschainIndexerState>>,
    init_timestamp: chrono::NaiveDateTime,
    error_count: Arc<AtomicU64>,
}

struct RunContext {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    chains: Vec<AmbChainConfig>,
    abi_registry: Arc<AbiRegistry>,
    message_hash_lookup: Arc<DashMap<B256, Key>>,
    pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>>,
    settings: AmbIndexerSettings,
    buffer: Arc<MessageBuffer<Message>>,
}

impl AmbIndexer {
    pub fn new(
        stats: Arc<StatsService>,
        bridge_id: i32,
        chains: Vec<AmbChainConfig>,
        settings: &AmbIndexerSettings,
        buffer_settings: &MessageBufferSettings,
    ) -> Result<Self> {
        ensure!(
            !chains.is_empty(),
            "AMB indexer requires at least one chain"
        );

        settings
            .failure_retry
            .validate()
            .context("invalid AMB indexer failure_retry settings")?;

        let abi_registry = Arc::new(AbiRegistry::from_chains(&chains)?);
        let db = stats.interchain_db_arc();
        let buffer = MessageBuffer::new_with_stats(stats, buffer_settings.clone());

        Ok(Self {
            db,
            bridge_id,
            chains,
            abi_registry,
            message_hash_lookup: Arc::new(DashMap::new()),
            pending_message_hash_events: Arc::new(DashMap::new()),
            settings: settings.clone(),
            buffer,
            buffer_handle: Arc::new(parking_lot::RwLock::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
            indexing_handle: Arc::new(parking_lot::RwLock::new(None)),
            state: Arc::new(parking_lot::RwLock::new(CrosschainIndexerState::Idle)),
            init_timestamp: chrono::Utc::now().naive_utc(),
            error_count: Arc::new(AtomicU64::new(0)),
        })
    }

    fn run_context(&self) -> RunContext {
        RunContext {
            db: self.db.clone(),
            bridge_id: self.bridge_id,
            chains: self.chains.clone(),
            abi_registry: self.abi_registry.clone(),
            message_hash_lookup: self.message_hash_lookup.clone(),
            pending_message_hash_events: self.pending_message_hash_events.clone(),
            settings: self.settings.clone(),
            buffer: self.buffer.clone(),
        }
    }

    async fn run(ctx: RunContext) -> Result<()> {
        tracing::info!(
            bridge_id = ctx.bridge_id,
            chain_count = ctx.chains.len(),
            "starting AMB indexer"
        );

        let mut streams = Vec::with_capacity(ctx.chains.len());
        for chain in &ctx.chains {
            let chain_id = chain.chain_id;
            let filter = ctx.abi_registry.filter_for_chain(chain_id)?;
            let stream = crate::indexer::evm::build_log_stream_for_chain(
                chain.provider.clone(),
                chain_id,
                ctx.bridge_id,
                filter,
                chain.start_block,
                &ctx.db,
                ctx.settings.pull_interval_ms,
                ctx.settings.batch_size,
            )
            .await?;
            streams.push((chain_id, stream));
        }

        let ledger = Arc::new(FailureLedger::new(ctx.db.clone()));
        let failure_retry_settings = ctx.settings.failure_retry.clone();

        RangeDriver::new(ctx, ledger, failure_retry_settings)
            .run(streams)
            .await
    }

    /// Returns `Err(BatchError)` narrowed to the block(s) of any transaction
    /// that failed to process, rather than swallowing the error and always
    /// returning `Ok(())`. Swallowing it would let the retry path's
    /// `ledger.resolve` delete the recorded hole for a range whose
    /// transactions actually failed — the messages would be lost *and* the
    /// ledger would report the range recovered.
    async fn process_batch(
        ctx: &RunContext,
        chain_id: i64,
        provider: &DynProvider<Ethereum>,
        batch: &[Log],
    ) -> Result<(), BatchError> {
        let logs_by_tx = crate::indexer::evm::group_logs_by_transaction(batch);
        let hashes = logs_by_tx.keys().copied().collect::<Vec<_>>();
        let receipts = crate::indexer::evm::fetch_receipts_for_transactions(
            provider,
            hashes,
            ctx.settings.receipt_concurrency as usize,
        )
        .await?;

        let mut failed_blocks: Vec<u64> = Vec::new();
        let mut last_err: Option<anyhow::Error> = None;
        let mut failed_count = 0usize;

        for (hash, logs) in logs_by_tx {
            let Some(receipt) = receipts.get(&hash) else {
                // Unreachable while `fetch_receipts_for_transactions` is a
                // `try_collect` over exactly these hashes — a missing receipt
                // is an `Err` there, not an absent key. It is still a failure
                // and not a skip: dropping these logs silently would let the
                // checkpoint advance past their blocks, and a retry pass would
                // `resolve` the hole for a range whose logs were never
                // dispatched. Attribute it to the logs' own blocks; an empty
                // `attributed` falls back to the whole yielded range.
                tracing::warn!(
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    log_count = logs.len(),
                    "missing AMB receipt for transaction; attributing its blocks as failed"
                );
                failed_blocks.extend(logs.iter().filter_map(|log| log.block_number));
                failed_count += 1;
                last_err = Some(anyhow!(
                    "missing receipt for AMB transaction {hash} on chain {chain_id}"
                ));
                continue;
            };
            let event_ctx = EventContext {
                bridge_id: ctx.bridge_id,
                chain_id,
                block_number: receipt.block.header.number,
                abi_registry: &ctx.abi_registry,
                buffer: &ctx.buffer,
                message_hash_lookup: &ctx.message_hash_lookup,
                pending_message_hash_events: &ctx.pending_message_hash_events,
                settings: &ctx.settings,
            };
            if let Err(err) = events::dispatch_transaction(
                &event_ctx,
                &receipt.logs,
                &receipt.block,
                receipt.transaction_from,
            )
            .await
            {
                // Downgraded to `warn`: the aggregate failure is now
                // propagated (below) and logged once, at `error`, by the
                // driver for the whole range — logging every per-tx failure
                // at `error` too would double-log the same incident.
                tracing::warn!(
                    err = ?err,
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    "failed to dispatch AMB transaction"
                );
                failed_blocks.push(receipt.block.header.number);
                failed_count += 1;
                last_err = Some(err);
            }
        }

        if let Some(err) = last_err {
            failed_blocks.sort_unstable();
            failed_blocks.dedup();
            let attributed = failed_blocks
                .into_iter()
                .map(|number| BlockRange {
                    from: number,
                    to: number,
                })
                .collect();

            return Err(BatchError {
                error: err.context(format!(
                    "{failed_count} AMB transaction(s) failed to process"
                )),
                attributed,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl RangeProcessor for RunContext {
    fn bridge_id(&self) -> i32 {
        self.bridge_id
    }

    fn chain_ids(&self) -> Vec<i64> {
        self.chains.iter().map(|c| c.chain_id).collect()
    }

    fn provider(&self, chain_id: i64) -> Option<DynProvider<Ethereum>> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| c.provider.clone())
    }

    fn log_filter(&self, chain_id: i64) -> Result<Filter> {
        self.abi_registry.filter_for_chain(chain_id)
    }

    fn batch_size(&self) -> u64 {
        self.settings.batch_size
    }

    async fn process(&self, chain_id: i64, batch: &LogBatch) -> Result<(), BatchError> {
        // The stream never yields an empty range (`log_stream.rs`), but the
        // retry path can, and this guard is cheap to keep for either case.
        if batch.logs.is_empty() {
            return Ok(());
        }

        let provider = self
            .provider(chain_id)
            .ok_or_else(|| anyhow!("no provider configured for chain_id {chain_id}"))?;

        AmbIndexer::process_batch(self, chain_id, &provider, &batch.logs).await
    }
}

#[async_trait]
impl CrosschainIndexer for AmbIndexer {
    fn name(&self) -> String {
        "AMB\\Omnibridge".into()
    }

    fn description(&self) -> String {
        "AMB \\ Omnibridge indexer".into()
    }

    async fn start(&self) -> Result<()> {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            tracing::debug!(bridge_id = self.bridge_id, "AMB indexer already running");
            return Ok(());
        }

        *self.state.write() = CrosschainIndexerState::Running;

        let buffer_handle = match self.buffer.clone().start().await {
            Ok(handle) => handle,
            Err(err) => {
                self.is_running.store(false, Ordering::Release);
                *self.state.write() = CrosschainIndexerState::Idle;
                return Err(err);
            }
        };
        *self.buffer_handle.write() = Some(buffer_handle);

        let run_ctx = self.run_context();
        let guard = crate::indexer::cleanup_guard::CleanupGuard {
            is_running: self.is_running.clone(),
            state: self.state.clone(),
            buffer_handle: self.buffer_handle.clone(),
            indexing_handle: self.indexing_handle.clone(),
            bridge_id: self.bridge_id,
        };
        let state = self.state.clone();
        let error_count = self.error_count.clone();
        let bridge_id = self.bridge_id;
        let is_running = self.is_running.clone();

        let handle = tokio::spawn(async move {
            let _guard = guard;

            if !is_running.load(Ordering::Acquire) {
                return;
            }

            if let Err(err) = Self::run(run_ctx).await {
                error_count.fetch_add(1, Ordering::Relaxed);
                // Redact once and use the same string for both sinks — see the
                // matching comment in the Avalanche indexer: `?err` renders a
                // transport error's cause chain, URL included, so an unredacted
                // log here defeats the redaction applied to the state.
                let redacted = redact_urls(&format!("{err:#}"));
                tracing::error!(err = %redacted, bridge_id, "AMB indexer task stopped with error");
                *state.write() = CrosschainIndexerState::Failed(redacted);
            }
        });

        *self.indexing_handle.write() = Some(handle);
        Ok(())
    }

    async fn stop(&self) {
        self.is_running.store(false, Ordering::Release);
        if let Some(handle) = self.indexing_handle.write().take() {
            handle.abort();
        }
        if let Some(handle) = self.buffer_handle.write().take() {
            handle.abort();
        }
        *self.state.write() = CrosschainIndexerState::Idle;
    }

    fn get_state(&self) -> CrosschainIndexerState {
        self.state.read().clone()
    }

    fn get_status(&self) -> CrosschainIndexerStatus {
        let mediator_versions = self
            .chains
            .iter()
            .flat_map(|chain| chain.mediators.iter().map(|mediator| mediator.version))
            .collect::<Vec<_>>();
        let extra_info = HashMap::from([
            (
                "chains_count".to_string(),
                serde_json::json!(self.chains.len()),
            ),
            (
                "poll_interval_ms".to_string(),
                serde_json::json!(self.settings.pull_interval_ms.as_millis()),
            ),
            (
                "batch_size".to_string(),
                serde_json::json!(self.settings.batch_size),
            ),
            (
                "receipt_concurrency".to_string(),
                serde_json::json!(self.settings.receipt_concurrency),
            ),
            (
                "mediator_versions".to_string(),
                serde_json::json!(mediator_versions),
            ),
        ]);

        CrosschainIndexerStatus {
            state: self.state.read().clone(),
            init_timestamp: self.init_timestamp,
            extra_info,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for the P1 review finding: `dispatch_transaction`
    //! (`events.rs`) used to log every handler failure and unconditionally
    //! return `Ok(())`, which made `process_batch`'s `failed_blocks`/
    //! `last_err` collection below dead code. These tests exercise the real
    //! `RunContext` -> `RangeDriver` pipeline (not a reimplementation of it)
    //! so a regression in `dispatch_transaction`'s aggregation is caught here,
    //! not just in `events.rs`'s own unit tests.

    use std::{
        future,
        task::{Context as TaskContext, Poll},
        time::Duration,
    };

    use alloy::{
        consensus::{Eip658Value, Receipt, ReceiptEnvelope, ReceiptWithBloom},
        json_abi::Event,
        primitives::{Bytes, LogData, address},
        providers::{Provider, ProviderBuilder},
        rpc::{
            client::RpcClient,
            types::{Block, Header as RpcHeader, TransactionReceipt},
        },
        transports::{TransportError, TransportErrorKind, TransportFut},
    };
    use alloy_json_rpc::{Id, RequestPacket, Response, ResponsePacket, ResponsePayload};
    use tower::Service;

    use super::*;
    use crate::{
        MessageBufferSettings,
        indexer::{
            amb::abi::{ContractAbi, ContractKind, ContractVersion},
            failure_ledger::{BlockRange, FailedInterval, FailureLedger, FailureRetrySettings},
        },
        log_stream::ScanDirection,
        test_utils::{init_db, mock_db::fill_mock_interchain_database},
    };

    /// An event with one indexed and one non-indexed parameter, dispatched
    /// through `handle_validator_confirmation` — chosen because that handler
    /// decodes the log before touching the buffer or any correlation map, so
    /// a malformed log fails deterministically and immediately, with no other
    /// side effects to account for.
    fn signed_for_affirmation_event() -> alloy::json_abi::Event {
        serde_json::from_str(
            r#"{
                "anonymous": false,
                "inputs": [
                    {"indexed": true,  "name": "signer",      "type": "address"},
                    {"indexed": false, "name": "messageHash", "type": "bytes32"}
                ],
                "name": "SignedForAffirmation",
                "type": "event"
            }"#,
        )
        .expect("SignedForAffirmation ABI")
    }

    fn registry_with_event(chain_id: i64, contract_address: Address, event: Event) -> AbiRegistry {
        let mut events_by_topic = HashMap::new();
        events_by_topic.insert(event.selector(), event);
        AbiRegistry::from_contracts_for_test(vec![ContractAbi {
            chain_id,
            address: contract_address,
            versions: vec![ContractVersion {
                started_at_block: 0,
                kind: ContractKind::OmnibridgeMediator,
                events_by_topic,
            }],
        }])
    }

    /// A log matching `event`'s topic0 but missing the indexed `signer`
    /// topic, so `Event::decode_log` fails with `TopicLengthMismatch` —
    /// deterministic, and locally detectable without a real chain.
    fn undecodable_log(contract_address: Address, event: &Event, tx_hash: B256, block: u64) -> Log {
        Log {
            inner: alloy::primitives::Log {
                address: contract_address,
                data: LogData::new_unchecked(vec![event.selector()], Bytes::new()),
            },
            block_number: Some(block),
            transaction_hash: Some(tx_hash),
            log_index: Some(0),
            ..Default::default()
        }
    }

    fn mock_receipt(
        tx_hash: B256,
        block_number: u64,
        log: Log,
        from: Address,
        to: Address,
    ) -> TransactionReceipt {
        let envelope = ReceiptEnvelope::Legacy(ReceiptWithBloom {
            receipt: Receipt {
                status: Eip658Value::Eip658(true),
                cumulative_gas_used: 21_000,
                logs: vec![log],
            },
            logs_bloom: Default::default(),
        });

        TransactionReceipt {
            inner: envelope,
            transaction_hash: tx_hash,
            transaction_index: Some(0),
            block_hash: Some(B256::with_last_byte(1)),
            block_number: Some(block_number),
            gas_used: 21_000,
            effective_gas_price: 1,
            blob_gas_used: None,
            blob_gas_price: None,
            from,
            to: Some(to),
            contract_address: None,
        }
    }

    fn mock_block(block_number: u64, timestamp: u64) -> Block {
        let header = alloy::consensus::Header {
            number: block_number,
            timestamp,
            ..Default::default()
        };
        Block {
            header: RpcHeader::new(header),
            uncles: vec![],
            transactions: Default::default(),
            withdrawals: None,
        }
    }

    /// Serves `eth_getLogs` (retry path only), `eth_getTransactionReceipt`
    /// and `eth_getBlockByNumber` — the only RPC methods `process_batch`
    /// (and, on retry, `fetch_logs`) issue. Fixed canned responses, not a
    /// queue: every call to a given method gets the same answer, which is
    /// all these tests need.
    #[derive(Clone)]
    struct AmbDispatchMockService {
        logs: Vec<Log>,
        receipt: TransactionReceipt,
        block: Block,
    }

    impl Service<RequestPacket> for AmbDispatchMockService {
        type Response = ResponsePacket;
        type Error = TransportError;
        type Future = TransportFut<'static>;

        fn poll_ready(&mut self, _cx: &mut TaskContext<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: RequestPacket) -> Self::Future {
            let single = req.as_single();
            let result = match single {
                Some(sr) => match sr.method() {
                    "eth_getLogs" => Ok(json_response(sr.id().clone(), &self.logs)),
                    "eth_getTransactionReceipt" => {
                        Ok(json_response(sr.id().clone(), &self.receipt))
                    }
                    "eth_getBlockByNumber" => Ok(json_response(sr.id().clone(), &self.block)),
                    other => Err(TransportErrorKind::custom_str(&format!(
                        "AmbDispatchMockService: unexpected method {other}"
                    ))),
                },
                None => Err(TransportErrorKind::custom_str(
                    "AmbDispatchMockService: expected a single request",
                )),
            };
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

    fn mock_provider(service: AmbDispatchMockService) -> DynProvider<Ethereum> {
        let client = RpcClient::builder().transport(service, false);
        ProviderBuilder::new().connect_client(client).erased()
    }

    fn chain_config(
        chain_id: i64,
        contract_address: Address,
        provider: DynProvider<Ethereum>,
    ) -> AmbChainConfig {
        AmbChainConfig {
            chain_id,
            provider,
            start_block: 0,
            amb_proxies: vec![AmbContractConfig {
                address: contract_address,
                version: 6,
                started_at_block: 0,
                abi: None,
            }],
            mediators: Vec::new(),
        }
    }

    fn run_context(
        db: Arc<InterchainDatabase>,
        owned_db: InterchainDatabase,
        bridge_id: i32,
        chains: Vec<AmbChainConfig>,
        registry: AbiRegistry,
    ) -> RunContext {
        RunContext {
            db,
            bridge_id,
            chains,
            abi_registry: Arc::new(registry),
            message_hash_lookup: Arc::new(DashMap::new()),
            pending_message_hash_events: Arc::new(DashMap::new()),
            settings: AmbIndexerSettings::default(),
            buffer: MessageBuffer::<Message>::new(
                owned_db,
                MessageBufferSettings {
                    hot_ttl: Duration::from_secs(60),
                    maintenance_interval: Duration::from_secs(60),
                },
            ),
        }
    }

    /// Acceptance: an AMB handler error must propagate all the way into an
    /// `indexer_failures` row for the correct block. Before the fix,
    /// `dispatch_transaction` always returned `Ok(())`, so this batch would
    /// have looked successful and no row would ever appear.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn amb_handler_failure_creates_indexer_failure_row_for_the_failing_block() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;
        const BLOCK_NUMBER: u64 = 500;

        let db = init_db("amb_handler_failure_creates_indexer_failure_row").await;
        fill_mock_interchain_database(&db).await;
        let owned_db = InterchainDatabase::new(db.client());
        let arc_db = Arc::new(owned_db.clone());
        let ledger = Arc::new(FailureLedger::new(arc_db.clone()));

        let contract_address = address!("1111111111111111111111111111111111111111");
        let tx_hash = B256::with_last_byte(7);
        let event = signed_for_affirmation_event();
        let registry = registry_with_event(CHAIN_ID, contract_address, event.clone());
        let log = undecodable_log(contract_address, &event, tx_hash, BLOCK_NUMBER);
        let receipt = mock_receipt(
            tx_hash,
            BLOCK_NUMBER,
            log.clone(),
            Address::ZERO,
            contract_address,
        );
        let service = AmbDispatchMockService {
            logs: vec![],
            receipt,
            block: mock_block(BLOCK_NUMBER, 1_700_000_000),
        };
        let chain = chain_config(CHAIN_ID, contract_address, mock_provider(service));
        let ctx = run_context(arc_db.clone(), owned_db, BRIDGE_ID, vec![chain], registry);

        let batch = LogBatch {
            from_block: BLOCK_NUMBER,
            to_block: BLOCK_NUMBER,
            direction: ScanDirection::Realtime,
            logs: vec![log],
        };
        let stream = futures::stream::iter(vec![(CHAIN_ID, batch)]);
        let settings = FailureRetrySettings {
            enabled: false,
            ..Default::default()
        };

        RangeDriver::new(ctx, ledger, settings)
            .run(vec![(CHAIN_ID, stream)])
            .await
            .expect("a recordable handler failure must not escalate the driver");

        let open = arc_db
            .open_indexer_failures(&[(BRIDGE_ID, CHAIN_ID)])
            .await
            .unwrap();
        assert_eq!(
            open.len(),
            1,
            "the AMB handler failure must create exactly one open interval: {open:?}"
        );
        assert_eq!(
            open[0].2.range,
            BlockRange {
                from: BLOCK_NUMBER,
                to: BLOCK_NUMBER
            },
            "the failing transaction's block must be recorded: {open:?}"
        );
    }

    /// Regression for the false-clear: a *repeated* handler failure during a
    /// retry pass must re-record the block, never resolve it. If
    /// `dispatch_transaction` goes back to swallowing handler errors, this
    /// batch looks successful to `retry_pending`, which calls
    /// `ledger.resolve` and deletes the still-real hole — this test fails in
    /// exactly that scenario.
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn repeated_amb_handler_failure_during_retry_does_not_resolve_existing_hole() {
        const BRIDGE_ID: i32 = 1;
        const CHAIN_ID: i64 = 1;
        const BLOCK_NUMBER: u64 = 700;

        let db = init_db("amb_repeated_handler_failure_no_resolve").await;
        fill_mock_interchain_database(&db).await;
        let owned_db = InterchainDatabase::new(db.client());
        let arc_db = Arc::new(owned_db.clone());
        let ledger = Arc::new(FailureLedger::new(arc_db.clone()));

        arc_db
            .record_indexer_failures(
                BRIDGE_ID,
                CHAIN_ID,
                &[(
                    BlockRange {
                        from: BLOCK_NUMBER,
                        to: BLOCK_NUMBER,
                    },
                    "seed".to_string(),
                )],
            )
            .await
            .unwrap();
        ledger.initialize(&[(BRIDGE_ID, CHAIN_ID)]).await.unwrap();

        let contract_address = address!("2222222222222222222222222222222222222222");
        let tx_hash = B256::with_last_byte(9);
        let event = signed_for_affirmation_event();
        let registry = registry_with_event(CHAIN_ID, contract_address, event.clone());
        let log = undecodable_log(contract_address, &event, tx_hash, BLOCK_NUMBER);
        let receipt = mock_receipt(
            tx_hash,
            BLOCK_NUMBER,
            log.clone(),
            Address::ZERO,
            contract_address,
        );
        let service = AmbDispatchMockService {
            logs: vec![log],
            receipt,
            block: mock_block(BLOCK_NUMBER, 1_700_000_000),
        };
        let chain = chain_config(CHAIN_ID, contract_address, mock_provider(service));
        let ctx = run_context(arc_db.clone(), owned_db, BRIDGE_ID, vec![chain], registry);

        let due: Vec<(i64, FailedInterval)> = ledger
            .open(&[(BRIDGE_ID, CHAIN_ID)])
            .await
            .unwrap()
            .into_iter()
            .map(|(_, chain_id, interval)| (chain_id, interval))
            .collect();
        assert_eq!(due.len(), 1, "the seeded hole must be due for retry");

        <RunContext as RangeProcessor>::retry_pending(&ctx, &ledger, &due, 16, &mut None).await;

        let open = ledger.open(&[(BRIDGE_ID, CHAIN_ID)]).await.unwrap();
        assert_eq!(
            open.len(),
            1,
            "a repeated handler failure must NOT resolve the existing hole: {open:?}"
        );
        assert_eq!(
            open[0].2.range,
            BlockRange {
                from: BLOCK_NUMBER,
                to: BLOCK_NUMBER
            }
        );
    }
}
