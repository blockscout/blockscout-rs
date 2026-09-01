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
use interchain_indexer_entity::tokens;
use sea_orm::ActiveValue;
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
    settings::XDaiIndexerSettings,
    types::{Message, NATIVE_SENTINEL},
    version::{XDaiSide, grammar_for},
};

/// Gnosis, the chain the native sentinel's `tokens` row is seeded on. Not
/// derived from `Direction` here: this seed runs once at startup, before any
/// message has established a direction, and the value is fixed by the
/// protocol (xDai's Home chain), not by which message happens to be seen
/// first.
const GNOSIS_CHAIN_ID: i64 = 100;

/// One configured deployment of the xDai proxy on one chain, valid from
/// `started_at_block` until the next version of the same address begins.
#[derive(Clone, Debug)]
pub struct XDaiContractConfig {
    pub address: Address,
    pub version: i16,
    pub started_at_block: u64,
    pub abi: Option<Value>,
}

/// One chain this bridge indexes, with every configured version of its
/// single proxy. xDai has exactly one contract kind per chain (unlike AMB's
/// proxy + mediator pair), so — unlike `AmbChainConfig` — there is only one
/// contract list.
#[derive(Clone)]
pub struct XDaiChainConfig {
    pub chain_id: i64,
    pub provider: DynProvider<Ethereum>,
    /// The lowest `started_at_block` among `contracts`.
    pub start_block: u64,
    /// At least one; xDai cannot index a chain without its proxy.
    pub contracts: Vec<XDaiContractConfig>,
}

pub struct XDaiIndexer {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    chains: Vec<XDaiChainConfig>,
    abi_registry: Arc<AbiRegistry>,
    /// Gno→Eth only: the Foreign proxy's own address, needed to compute
    /// `messageHash`. Resolved once at construction rather than per event.
    foreign_bridge_address: Address,
    message_hash_lookup: Arc<DashMap<B256, Key>>,
    pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>>,
    settings: XDaiIndexerSettings,
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
    chains: Vec<XDaiChainConfig>,
    abi_registry: Arc<AbiRegistry>,
    foreign_bridge_address: Address,
    message_hash_lookup: Arc<DashMap<B256, Key>>,
    pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>>,
    settings: XDaiIndexerSettings,
    buffer: Arc<MessageBuffer<Message>>,
}

impl XDaiIndexer {
    pub fn new(
        stats: Arc<StatsService>,
        bridge_id: i32,
        chains: Vec<XDaiChainConfig>,
        settings: &XDaiIndexerSettings,
        buffer_settings: &MessageBufferSettings,
    ) -> Result<Self> {
        ensure!(
            !chains.is_empty(),
            "xDai indexer requires at least one chain"
        );

        settings
            .failure_retry
            .validate()
            .context("invalid xDai indexer failure_retry settings")?;

        let abi_registry = Arc::new(AbiRegistry::from_chains(&chains)?);
        let foreign_bridge_address = abi_registry.foreign_proxy_address()?;
        let db = stats.interchain_db_arc();
        let buffer = MessageBuffer::new_with_stats(stats, buffer_settings.clone());

        Ok(Self {
            db,
            bridge_id,
            chains,
            abi_registry,
            foreign_bridge_address,
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
            foreign_bridge_address: self.foreign_bridge_address,
            message_hash_lookup: self.message_hash_lookup.clone(),
            pending_message_hash_events: self.pending_message_hash_events.clone(),
            settings: self.settings.clone(),
            buffer: self.buffer.clone(),
        }
    }

    /// Seeds the `(100, 0x00…00)` sentinel `tokens` row this indexer's own
    /// transfers depend on for stats eligibility. Lives here, not in
    /// `server::run`, which must not know indexer specifics -- mirrors how
    /// `AmbIndexer::new` gets its DB handle from `stats.interchain_db_arc()`.
    ///
    /// Idempotent (`upsert_token_info`) and never fatal: a write failure is a
    /// `warn`, not a startup abort, the same rationale as
    /// `evm/log_stream_builder.rs::seed_catchup_floor` -- metadata enrichment
    /// must not be able to stop ingestion. See the gotcha in
    /// `.memory-bank/gotchas.md` for what a missing row actually costs
    /// (a permanent `Background token info fetch failed` warn stream, and a
    /// possible NULL `stats_asset_edges.decimals`), which is why this is not
    /// cosmetic despite being non-blocking.
    async fn seed_native_sentinel_token(&self) {
        let seed = tokens::ActiveModel {
            chain_id: ActiveValue::Set(GNOSIS_CHAIN_ID),
            address: ActiveValue::Set(NATIVE_SENTINEL.as_slice().to_vec()),
            symbol: ActiveValue::Set(Some("xDAI".to_string())),
            name: ActiveValue::Set(Some("xDai".to_string())),
            decimals: ActiveValue::Set(Some(18)),
            ..Default::default()
        };

        if let Err(err) = self.db.upsert_token_info(seed).await {
            tracing::warn!(
                err = ?err,
                bridge_id = self.bridge_id,
                chain_id = GNOSIS_CHAIN_ID,
                "failed to seed the native xDAI sentinel tokens row; stats enrichment will \
                 keep re-attempting a doomed fetch against it until this succeeds"
            );
        }
    }

    async fn run(ctx: RunContext) -> Result<()> {
        tracing::info!(
            bridge_id = ctx.bridge_id,
            chain_count = ctx.chains.len(),
            "starting xDai indexer"
        );

        check_source_asset_matches_latest(&ctx.chains, &ctx.abi_registry).await;

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
    /// transactions actually failed.
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
                tracing::warn!(
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    log_count = logs.len(),
                    "missing xDai receipt for transaction; attributing its blocks as failed"
                );
                failed_blocks.extend(logs.iter().filter_map(|log| log.block_number));
                failed_count += 1;
                last_err = Some(anyhow!(
                    "missing receipt for xDai transaction {hash} on chain {chain_id}"
                ));
                continue;
            };
            let event_ctx = EventContext {
                bridge_id: ctx.bridge_id,
                chain_id,
                block_number: receipt.block.header.number,
                abi_registry: &ctx.abi_registry,
                buffer: &ctx.buffer,
                foreign_bridge_address: ctx.foreign_bridge_address,
                message_hash_lookup: &ctx.message_hash_lookup,
                pending_message_hash_events: &ctx.pending_message_hash_events,
            };
            if let Err(err) = events::dispatch_transaction(
                &event_ctx,
                &receipt.logs,
                &receipt.block,
                receipt.transaction_from,
            )
            .await
            {
                tracing::warn!(
                    err = ?err,
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    "failed to dispatch xDai transaction"
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
                    "{failed_count} xDai transaction(s) failed to process"
                )),
                attributed,
            });
        }

        Ok(())
    }
}

alloy::sol! {
    #[sol(rpc)]
    interface IXDaiForeignBridge {
        function erc20token() external view returns (address);
    }
}

/// One-off startup sanity check, not a per-block probe: reads the Foreign
/// proxy's `erc20token()` at `latest` and compares it against the *newest*
/// configured version's `source_asset`. A mismatch means the bridge was
/// upgraded without a matching `bridges.json` update -- loud, but not fatal,
/// since a fresh vNext deployment should not stop the service.
///
/// Only the newest window is checked: historical windows are immutable and
/// were verified by bisection, and reading `erc20token()` at `latest` says
/// nothing about what it returned at a historical block.
async fn check_source_asset_matches_latest(chains: &[XDaiChainConfig], abi_registry: &AbiRegistry) {
    let Ok(foreign_chain_id) = abi_registry.chain_id_for_side(XDaiSide::Foreign) else {
        return;
    };
    let Some(chain) = chains.iter().find(|c| c.chain_id == foreign_chain_id) else {
        return;
    };
    let Some(newest) = chain
        .contracts
        .iter()
        .max_by_key(|contract| contract.started_at_block)
    else {
        return;
    };
    let expected = match grammar_for(XDaiSide::Foreign, newest.version) {
        Ok(grammar) => grammar.source_asset,
        Err(err) => {
            tracing::warn!(err = ?err, version = newest.version, "no xDai grammar for configured Foreign version; skipping source-asset sanity check");
            return;
        }
    };
    let Some(expected) = expected else {
        return;
    };

    let contract = IXDaiForeignBridge::new(newest.address, chain.provider.clone());
    match contract.erc20token().call().await {
        Ok(found) if found == expected => {}
        Ok(found) => tracing::warn!(
            chain_id = chain.chain_id,
            address = %newest.address,
            expected = %expected,
            found = %found,
            "xDai Foreign proxy erc20token() does not match the configured newest source_asset; \
             a bridge upgrade may have shipped without a matching bridges.json update"
        ),
        Err(err) => tracing::warn!(
            err = ?err,
            chain_id = chain.chain_id,
            address = %newest.address,
            "failed to sanity-check xDai Foreign proxy erc20token() against the configured \
             source_asset"
        ),
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
        if batch.logs.is_empty() {
            return Ok(());
        }

        let provider = self
            .provider(chain_id)
            .ok_or_else(|| anyhow!("no provider configured for chain_id {chain_id}"))?;

        XDaiIndexer::process_batch(self, chain_id, &provider, &batch.logs).await
    }
}

#[async_trait]
impl CrosschainIndexer for XDaiIndexer {
    fn name(&self) -> String {
        "xDai Bridge".into()
    }

    fn description(&self) -> String {
        "xDai bridge indexer".into()
    }

    async fn start(&self) -> Result<()> {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            tracing::debug!(bridge_id = self.bridge_id, "xDai indexer already running");
            return Ok(());
        }

        *self.state.write() = CrosschainIndexerState::Running;

        self.seed_native_sentinel_token().await;

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
                let redacted = redact_urls(&format!("{err:#}"));
                tracing::error!(err = %redacted, bridge_id, "xDai indexer task stopped with error");
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
    //! DB-backed replay tests. Log fixtures are hand-encoded (not fetched
    //! from chain) because `.memory-bank/research/xdai-bridge-protocol-and-indexing-fit.md`
    //! records the worked Eth→Gno trace's block numbers, the nonce (`0x1ae0`)
    //! and the transferred value in full, but its addresses and transaction
    //! hashes only truncated -- so those are synthetic placeholders here,
    //! while every value the note gives in full is reproduced exactly. What
    //! is under test either way: `dispatch_transaction` -> `MessageBuffer` ->
    //! `Consolidate` end to end, confirmations arriving across separate
    //! transactions/blocks, and `AffirmationCompleted` sharing a transaction
    //! with the last `SignedForAffirmation` -- the trace's actual shape.

    use std::time::Duration;

    use alloy::{
        json_abi::{Event, JsonAbi},
        primitives::{Address, B256, Bytes, LogData, U256, address},
        providers::{Provider, ProviderBuilder},
        rpc::types::Log,
    };
    use interchain_indexer_entity::{
        amb_messages_confirmations, bridges, chains, crosschain_messages, crosschain_transfers,
        sea_orm_active_enums::MessageStatus,
    };
    use sea_orm::{ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, TransactionTrait};

    use super::*;
    use crate::{
        IndexedChains, MessageBufferSettings,
        indexer::xdai::types::{Message, compute_message_hash, key_from_native_id, native_id_blob},
        message_buffer::MessageBuffer,
        test_utils::init_db,
    };

    const BRIDGE_ID: i32 = 3;
    const ETH: i64 = 1;
    const GNO: i64 = 100;

    fn dummy_provider() -> DynProvider<Ethereum> {
        ProviderBuilder::new()
            .connect_http("http://127.0.0.1:1".parse().unwrap())
            .erased()
    }

    fn foreign_abi() -> JsonAbi {
        serde_json::from_value(serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}
        ]))
        .expect("valid ABI")
    }

    fn home_abi() -> JsonAbi {
        serde_json::from_value(serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"},{"indexed":false,"name":"token","type":"address"}],"name":"UserRequestForSignature","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}
        ]))
        .expect("valid ABI")
    }

    fn event_of(abi: &JsonAbi, name: &str) -> Event {
        abi.events
            .get(name)
            .and_then(|events| events.first())
            .cloned()
            .expect("event present")
    }

    fn test_registry(foreign_addr: Address, home_addr: Address) -> AbiRegistry {
        let chains = vec![
            XDaiChainConfig {
                chain_id: ETH,
                provider: dummy_provider(),
                start_block: super::super::version::FOREIGN_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: foreign_addr,
                    version: 9,
                    started_at_block: super::super::version::FOREIGN_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(foreign_abi()).unwrap()),
                }],
            },
            XDaiChainConfig {
                chain_id: GNO,
                provider: dummy_provider(),
                start_block: super::super::version::HOME_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: home_addr,
                    version: 7,
                    started_at_block: super::super::version::HOME_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(home_abi()).unwrap()),
                }],
            },
        ];
        AbiRegistry::from_chains(&chains).expect("test registry builds")
    }

    fn word_address(a: Address) -> B256 {
        B256::left_padding_from(a.as_slice())
    }

    fn word_u256(v: U256) -> [u8; 32] {
        v.to_be_bytes::<32>()
    }

    fn make_log(
        emitter: Address,
        topics: Vec<B256>,
        data: Vec<u8>,
        tx_hash: B256,
        block_number: u64,
        log_index: u64,
    ) -> Log {
        Log {
            inner: alloy::primitives::Log {
                address: emitter,
                data: LogData::new_unchecked(topics, Bytes::from(data)),
            },
            transaction_hash: Some(tx_hash),
            block_number: Some(block_number),
            log_index: Some(log_index),
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn user_request_for_affirmation_log(
        event: &Event,
        emitter: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
        tx_hash: B256,
        block_number: u64,
    ) -> Log {
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(word_address(recipient).as_slice());
        data.extend_from_slice(&word_u256(value));
        data.extend_from_slice(&word_u256(nonce));
        make_log(
            emitter,
            vec![event.selector()],
            data,
            tx_hash,
            block_number,
            0,
        )
    }

    fn signed_for_affirmation_log(
        event: &Event,
        emitter: Address,
        signer: Address,
        nonce: U256,
        tx_hash: B256,
        block_number: u64,
        log_index: u64,
    ) -> Log {
        let topics = vec![event.selector(), word_address(signer)];
        let data = word_u256(nonce).to_vec();
        make_log(emitter, topics, data, tx_hash, block_number, log_index)
    }

    #[allow(clippy::too_many_arguments)]
    fn affirmation_completed_log(
        event: &Event,
        emitter: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
        tx_hash: B256,
        block_number: u64,
        log_index: u64,
    ) -> Log {
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(word_address(recipient).as_slice());
        data.extend_from_slice(&word_u256(value));
        data.extend_from_slice(&word_u256(nonce));
        make_log(
            emitter,
            vec![event.selector()],
            data,
            tx_hash,
            block_number,
            log_index,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn user_request_for_signature_log(
        event: &Event,
        emitter: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
        token: Option<Address>,
        tx_hash: B256,
        block_number: u64,
    ) -> Log {
        let mut data = Vec::with_capacity(128);
        data.extend_from_slice(word_address(recipient).as_slice());
        data.extend_from_slice(&word_u256(value));
        data.extend_from_slice(&word_u256(nonce));
        if let Some(token) = token {
            data.extend_from_slice(word_address(token).as_slice());
        }
        make_log(
            emitter,
            vec![event.selector()],
            data,
            tx_hash,
            block_number,
            0,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn collected_signatures_log(
        event: &Event,
        emitter: Address,
        authority: Address,
        message_hash: B256,
        count: U256,
        tx_hash: B256,
        block_number: u64,
        log_index: u64,
    ) -> Log {
        let mut data = Vec::with_capacity(96);
        data.extend_from_slice(word_address(authority).as_slice());
        data.extend_from_slice(message_hash.as_slice());
        data.extend_from_slice(&word_u256(count));
        make_log(
            emitter,
            vec![event.selector()],
            data,
            tx_hash,
            block_number,
            log_index,
        )
    }

    fn block_with_timestamp(ts: u64) -> alloy::rpc::types::Block {
        let mut block: alloy::rpc::types::Block = Default::default();
        block.header.timestamp = ts;
        block
    }

    async fn seed_bridge_and_chains(db: &InterchainDatabase) {
        bridges::Entity::insert(bridges::ActiveModel {
            id: Set(BRIDGE_ID),
            name: Set("xDai Bridge".to_string()),
            ..Default::default()
        })
        .exec(db.db.as_ref())
        .await
        .unwrap();
        chains::Entity::insert_many([
            chains::ActiveModel {
                id: Set(ETH),
                name: Set("Ethereum".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(GNO),
                name: Set("Gnosis".to_string()),
                ..Default::default()
            },
        ])
        .exec(db.db.as_ref())
        .await
        .unwrap();
    }

    /// Replays the verified Eth→Gno trace's real shape: one source request,
    /// four confirmations across separate Gnosis transactions/blocks, and
    /// `AffirmationCompleted` sharing its transaction with the fourth
    /// confirmation. Asserts one `Completed` message with the expected
    /// `native_id` and four confirmation rows.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn eth_to_gno_trace_replay_produces_one_completed_message_with_four_confirmations() {
        let db = init_db("xdai_eth_to_gno_trace_replay").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let foreign_event = event_of(&foreign_abi(), "UserRequestForAffirmation");
        let signed_event = event_of(&home_abi(), "SignedForAffirmation");
        let completed_event = event_of(&home_abi(), "AffirmationCompleted");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        // Real, fully-recorded facts from the research note's worked trace.
        let nonce = U256::from(0x1ae0_u64);
        let value = U256::from_str_radix("23673375455773347526", 10).unwrap();
        const SRC_BLOCK: u64 = 25_852_059;
        const CONF1_BLOCK: u64 = 47_953_922;
        const CONF2_BLOCK: u64 = 47_954_052;
        const CONF3_BLOCK: u64 = 47_954_055;
        const CONF4_COMPLETE_BLOCK: u64 = 47_954_055;

        // Synthetic (see module doc): the note only records these truncated.
        let recipient = Address::repeat_byte(0xC3);
        let sender = Address::repeat_byte(0x55);
        let validator1 = Address::repeat_byte(0xA1);
        let validator2 = Address::repeat_byte(0xA2);
        let validator3 = Address::repeat_byte(0xA3);
        let validator4 = Address::repeat_byte(0xA4);
        let tx_src = B256::repeat_byte(0x01);
        let tx_conf1 = B256::repeat_byte(0x02);
        let tx_conf2 = B256::repeat_byte(0x03);
        let tx_conf3 = B256::repeat_byte(0x04);
        let tx_conf4_complete = B256::repeat_byte(0x05);

        let source_log = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            SRC_BLOCK,
        );
        let conf1_log = signed_for_affirmation_log(
            &signed_event,
            home_addr,
            validator1,
            nonce,
            tx_conf1,
            CONF1_BLOCK,
            0,
        );
        let conf2_log = signed_for_affirmation_log(
            &signed_event,
            home_addr,
            validator2,
            nonce,
            tx_conf2,
            CONF2_BLOCK,
            0,
        );
        let conf3_log = signed_for_affirmation_log(
            &signed_event,
            home_addr,
            validator3,
            nonce,
            tx_conf3,
            CONF3_BLOCK,
            16,
        );
        let conf4_log = signed_for_affirmation_log(
            &signed_event,
            home_addr,
            validator4,
            nonce,
            tx_conf4_complete,
            CONF4_COMPLETE_BLOCK,
            18,
        );
        let completed_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            nonce,
            tx_conf4_complete,
            CONF4_COMPLETE_BLOCK,
            20,
        );

        let eth_block = block_with_timestamp(1_700_000_000);
        let gno_block_1 = block_with_timestamp(1_700_000_100);
        let gno_block_2 = block_with_timestamp(1_700_000_200);
        let gno_block_3 = block_with_timestamp(1_700_000_300);
        let gno_block_4 = block_with_timestamp(1_700_000_400);

        let src_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: SRC_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&src_ctx, &[source_log], &eth_block, sender)
            .await
            .expect("source dispatch succeeds");

        let conf1_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: CONF1_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&conf1_ctx, &[conf1_log], &gno_block_1, validator1)
            .await
            .expect("conf1 dispatch succeeds");

        let conf2_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: CONF2_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&conf2_ctx, &[conf2_log], &gno_block_2, validator2)
            .await
            .expect("conf2 dispatch succeeds");

        let conf3_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: CONF3_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&conf3_ctx, &[conf3_log], &gno_block_3, validator3)
            .await
            .expect("conf3 dispatch succeeds");

        let conf4_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: CONF4_COMPLETE_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(
            &conf4_ctx,
            &[conf4_log, completed_log],
            &gno_block_4,
            validator4,
        )
        .await
        .expect("conf4 + completion dispatch succeeds");

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(1, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("message row must exist");

        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(message.native_id, Some(native_id.to_vec()));
        assert_eq!(message.src_chain_id, ETH);
        assert_eq!(message.dst_chain_id, Some(GNO));
        assert_eq!(
            message.dst_tx_hash,
            Some(tx_conf4_complete.as_slice().to_vec())
        );

        let confirmations = amb_messages_confirmations::Entity::find()
            .filter(amb_messages_confirmations::Column::MessageId.eq(key.message_id))
            .filter(amb_messages_confirmations::Column::BridgeId.eq(BRIDGE_ID))
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(confirmations.len(), 4, "one row per validator");
    }

    /// `relayTokens` can be called in a loop by an aggregator, so one
    /// transaction can carry N source-request logs. Each must become its own
    /// message (`evm/transaction_grouping.rs` already groups logs by
    /// transaction; this pins that xDai's own dispatch does not collapse
    /// them).
    #[tokio::test]
    #[ignore = "needs database"]
    async fn two_user_request_for_affirmation_logs_in_one_transaction_produce_two_distinct_messages()
     {
        let db = init_db("xdai_two_logs_one_transaction").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let foreign_event = event_of(&foreign_abi(), "UserRequestForAffirmation");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        const BLOCK: u64 = 25_852_100;
        let tx = B256::repeat_byte(0x09);
        let sender = Address::repeat_byte(0x55);
        let nonce_a = U256::from(0x2001_u64);
        let nonce_b = U256::from(0x2002_u64);

        let log_a = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            Address::repeat_byte(0x11),
            U256::from(1_000u64),
            nonce_a,
            tx,
            BLOCK,
        );
        let log_b = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            Address::repeat_byte(0x22),
            U256::from(2_000u64),
            nonce_b,
            tx,
            BLOCK,
        );

        let block = block_with_timestamp(1_700_000_000);
        let ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&ctx, &[log_a, log_b], &block, sender)
            .await
            .expect("dispatch succeeds");

        buffer.run().await.expect("maintenance flush succeeds");

        let key_a = key_from_native_id(&native_id_blob(1, nonce_a).unwrap(), BRIDGE_ID).unwrap();
        let key_b = key_from_native_id(&native_id_blob(1, nonce_b).unwrap(), BRIDGE_ID).unwrap();
        assert_ne!(key_a.message_id, key_b.message_id);

        let row_a = crosschain_messages::Entity::find_by_id((key_a.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("message A must exist");
        let row_b = crosschain_messages::Entity::find_by_id((key_b.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("message B must exist");

        assert_eq!(row_a.status, MessageStatus::Initiated);
        assert_eq!(row_b.status, MessageStatus::Initiated);
    }

    /// Replays the verified Gno→Eth trace's real, fully-recorded facts: the
    /// nonce (`0x140a`), the transferred value, the DAI token, and both
    /// block numbers. Addresses and transaction hashes are synthetic
    /// placeholders (see the module doc) since the research note's Gno→Eth
    /// trace only records those truncated.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn gno_to_eth_trace_replay_produces_one_completed_message() {
        let db = init_db("xdai_gno_to_eth_trace_replay").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let signature_event = event_of(&home_abi(), "UserRequestForSignature");
        let relayed_event = event_of(&foreign_abi(), "RelayedMessage");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        // Real, fully-recorded facts from the research note's worked trace.
        let nonce = U256::from(0x140a_u64);
        let value = U256::from_str_radix("39239013587778384001516", 10).unwrap();
        let dai = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
        const SIGNATURE_BLOCK: u64 = 47_945_328;
        const RELAYED_BLOCK: u64 = 25_848_470;

        // Synthetic: the note only records these truncated.
        let recipient = Address::repeat_byte(0xA1);
        let sender = Address::repeat_byte(0x66);
        let tx_signature = B256::repeat_byte(0x06);
        let tx_relayed = B256::repeat_byte(0x07);

        let signature_log = user_request_for_signature_log(
            &signature_event,
            home_addr,
            recipient,
            value,
            nonce,
            Some(dai),
            tx_signature,
            SIGNATURE_BLOCK,
        );
        let relayed_log = affirmation_completed_log(
            &relayed_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_relayed,
            RELAYED_BLOCK,
            0,
        );

        let gno_block = block_with_timestamp(1_700_100_000);
        let eth_block = block_with_timestamp(1_700_200_000);

        let signature_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: SIGNATURE_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&signature_ctx, &[signature_log], &gno_block, sender)
            .await
            .expect("signature dispatch succeeds");

        let relayed_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: RELAYED_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(&relayed_ctx, &[relayed_log], &eth_block, Address::ZERO)
            .await
            .expect("relayed dispatch succeeds");

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(100, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("message row must exist");

        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(message.native_id, Some(native_id.to_vec()));
        assert_eq!(message.src_chain_id, GNO);
        assert_eq!(message.dst_chain_id, Some(ETH));
        assert_eq!(message.dst_tx_hash, Some(tx_relayed.as_slice().to_vec()));
        assert_eq!(message.sender_address, Some(sender.as_slice().to_vec()));
    }

    /// `SignedForUserRequest`/`CollectedSignatures` are same-chain (Gnosis)
    /// but catch-up and realtime scan concurrently, so a `CollectedSignatures`
    /// can arrive before its `UserRequestForSignature` source. This must
    /// queue, then drain once the source lands, reaching the exact same
    /// `ReadyToClaim` / unset-`dst_tx_hash` state as the in-order case
    /// (`consolidate_gno_to_eth_with_signatures_collected_is_ready_to_claim_with_no_dst_tx_hash`
    /// pins the same outcome at the unit level).
    #[tokio::test]
    #[ignore = "needs database"]
    async fn collected_signatures_before_its_source_is_queued_then_drained_to_ready_to_claim() {
        let db = init_db("xdai_collected_signatures_queued_then_drained").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let signature_event = event_of(&home_abi(), "UserRequestForSignature");
        let collected_event = event_of(&home_abi(), "CollectedSignatures");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x2400_u64);
        let value = U256::from(5_000u64);
        let recipient = Address::repeat_byte(0xB2);
        let sender = Address::repeat_byte(0x66);
        let authority = Address::repeat_byte(0xC4);
        // No verified on-chain `CollectedSignatures` instance exists for
        // xDai in the research note (its Gno→Eth trace has only
        // `UserRequestForSignature` and `RelayedMessage`) -- this log is
        // deliberately synthetic, per the task's explicit fallback for that
        // gap. The `messageHash` is computed the same way the indexer does,
        // so the queue/drain mechanics under test are exercised faithfully
        // even though the log itself was never observed on chain.
        let expected_message_hash =
            compute_message_hash(recipient, value, nonce, foreign_addr, Some(dai_address()));

        const SIGNATURE_BLOCK: u64 = 47_800_000;
        const COLLECTED_BLOCK: u64 = 47_800_010;
        let tx_signature = B256::repeat_byte(0x08);
        let tx_collected = B256::repeat_byte(0x09);

        let collected_log = collected_signatures_log(
            &collected_event,
            home_addr,
            authority,
            expected_message_hash,
            U256::from(4u64),
            tx_collected,
            COLLECTED_BLOCK,
            0,
        );
        let signature_log = user_request_for_signature_log(
            &signature_event,
            home_addr,
            recipient,
            value,
            nonce,
            Some(dai_address()),
            tx_signature,
            SIGNATURE_BLOCK,
        );

        let gno_block_collected = block_with_timestamp(1_700_300_000);
        let gno_block_signature = block_with_timestamp(1_700_300_100);

        // Out-of-order on purpose: CollectedSignatures dispatched first.
        let collected_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: COLLECTED_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(
            &collected_ctx,
            &[collected_log],
            &gno_block_collected,
            sender,
        )
        .await
        .expect("collected-signatures dispatch succeeds");
        assert_eq!(
            pending_message_hash_events.len(),
            1,
            "the queue must hold the event until its source arrives"
        );

        let signature_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: SIGNATURE_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(
            &signature_ctx,
            &[signature_log],
            &gno_block_signature,
            sender,
        )
        .await
        .expect("signature dispatch succeeds");

        assert!(
            pending_message_hash_events.is_empty(),
            "the queue must be drained once the source arrives"
        );

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(100, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("message row must exist");

        assert_eq!(message.status, MessageStatus::ReadyToClaim);
        assert_eq!(
            message.dst_tx_hash, None,
            "CollectedSignatures is a source-chain event, not a destination transaction"
        );
    }

    fn dai_address() -> Address {
        address!("6B175474E89094C44Da98b954EedeAC495271d0F")
    }

    /// Proves the native sentinel actually clears
    /// `transfer_identity_ready_condition` instead of deferring as
    /// `identity_incomplete`: a completed Gno→Eth transfer reaches
    /// `stats_processed = 1`, its two endpoints (Gnosis sentinel + Ethereum
    /// ERC-20) merge into one shared `stats_assets` row, and the resulting
    /// edge's `decimals` comes from the seeded sentinel row rather than
    /// ending up NULL.
    ///
    /// Gno→Eth specifically, not Eth→Gno: `amount_side` is sticky to
    /// whichever side is *source*-indexed
    /// (`stats/projection.rs`, "source_chain_indexed || src_dec.is_some()"),
    /// and for this direction the source chain is Gnosis -- so `decimals`
    /// is read from the sentinel's own seeded row, which is exactly the
    /// path this test exists to exercise. (For Eth→Gno the source side is
    /// the ERC-20, and decimals would instead depend on that token's row
    /// being enriched -- a real DAI/USDS contract eventually resolves that
    /// via `TokenInfoService`'s on-chain fetch, which is out of scope for
    /// this unit-level test.)
    #[tokio::test]
    #[ignore = "needs database"]
    async fn completed_transfer_reaches_one_shared_stats_asset_with_sentinel_decimals() {
        let db = init_db("xdai_stats_projection_sentinel").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        // Mirrors `XDaiIndexer::seed_native_sentinel_token`.
        interchain_db
            .upsert_token_info(interchain_indexer_entity::tokens::ActiveModel {
                chain_id: Set(GNO),
                address: Set(NATIVE_SENTINEL.as_slice().to_vec()),
                symbol: Set(Some("xDAI".to_string())),
                name: Set(Some("xDai".to_string())),
                decimals: Set(Some(18)),
                ..Default::default()
            })
            .await
            .expect("sentinel token seed succeeds");

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let signature_event = event_of(&home_abi(), "UserRequestForSignature");
        let relayed_event = event_of(&foreign_abi(), "RelayedMessage");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x3002_u64);
        let value = U256::from(9_000u64);
        let recipient = Address::repeat_byte(0xD2);
        let sender = Address::repeat_byte(0x77);
        let tx_signature = B256::repeat_byte(0x0C);
        let tx_relayed = B256::repeat_byte(0x0D);
        const SIGNATURE_BLOCK: u64 = 39_600_000;
        const RELAYED_BLOCK: u64 = 22_300_000;

        let signature_log = user_request_for_signature_log(
            &signature_event,
            home_addr,
            recipient,
            value,
            nonce,
            Some(dai_address()),
            tx_signature,
            SIGNATURE_BLOCK,
        );
        let relayed_log = affirmation_completed_log(
            &relayed_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_relayed,
            RELAYED_BLOCK,
            0,
        );

        let signature_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: SIGNATURE_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(
            &signature_ctx,
            &[signature_log],
            &block_with_timestamp(1_700_500_000),
            sender,
        )
        .await
        .expect("signature dispatch succeeds");

        let relayed_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: RELAYED_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
        };
        events::dispatch_transaction(
            &relayed_ctx,
            &[relayed_log],
            &block_with_timestamp(1_700_500_100),
            sender,
        )
        .await
        .expect("relayed dispatch succeeds");

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(100, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let transfer = crosschain_transfers::Entity::find()
            .filter(crosschain_transfers::Column::MessageId.eq(key.message_id))
            .filter(crosschain_transfers::Column::BridgeId.eq(BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("transfer row must exist");
        assert_eq!(
            transfer.token_src_address,
            Some(NATIVE_SENTINEL.as_slice().to_vec())
        );

        let conn = interchain_db.db.as_ref();
        conn.transaction::<_, (), sea_orm::DbErr>(|tx| {
            Box::pin(async move {
                crate::stats::projection::project_messages_batch(
                    tx,
                    &[(key.message_id, BRIDGE_ID)],
                    &IndexedChains::AllIndexed,
                )
                .await?;
                crate::stats::projection::project_transfers_batch(
                    tx,
                    &[transfer.id],
                    &IndexedChains::AllIndexed,
                )
                .await?;
                Ok(())
            })
        })
        .await
        .expect("projection succeeds");

        let projected = crosschain_transfers::Entity::find_by_id(transfer.id)
            .one(conn)
            .await
            .unwrap()
            .expect("transfer row must still exist");
        assert_eq!(projected.stats_processed, 1);
        let asset_id = projected
            .stats_asset_id
            .expect("identity must resolve, not defer");

        let asset_tokens: Vec<(i64, Vec<u8>)> =
            interchain_indexer_entity::stats_asset_tokens::Entity::find()
                .filter(
                    interchain_indexer_entity::stats_asset_tokens::Column::StatsAssetId
                        .eq(asset_id),
                )
                .all(conn)
                .await
                .unwrap()
                .into_iter()
                .map(|row| (row.chain_id, row.token_address))
                .collect();
        assert!(
            asset_tokens.contains(&(GNO, NATIVE_SENTINEL.as_slice().to_vec())),
            "the sentinel endpoint must be linked into the shared asset: {asset_tokens:?}"
        );
        assert!(
            asset_tokens.contains(&(ETH, dai_address().as_slice().to_vec())),
            "the Ethereum ERC-20 endpoint must be linked into the same asset: {asset_tokens:?}"
        );

        let edge = interchain_indexer_entity::stats_asset_edges::Entity::find_by_id((
            asset_id, GNO, ETH, BRIDGE_ID,
        ))
        .one(conn)
        .await
        .unwrap()
        .expect("edge row must exist");
        assert_eq!(
            edge.decimals,
            Some(18),
            "decimals must come from the seeded sentinel row, not end up NULL"
        );
    }
}
