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
use interchain_indexer_entity::{sea_orm_active_enums::TokenType, tokens};
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
    metrics,
    settings::XDaiIndexerSettings,
    types::{Message, NATIVE_SENTINEL},
    version::{XDaiSide, grammar_for},
};

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
    /// The chain the native sentinel's `tokens` row is seeded on: xDai's Home
    /// side, whichever chain id `bridges.json` puts there. Resolved at
    /// construction because the seed runs once at startup, before any message
    /// has established a direction.
    home_chain_id: i64,
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
        let home_chain_id = abi_registry.chain_id_for_side(XDaiSide::Home)?;
        let db = stats.interchain_db_arc();
        let buffer = MessageBuffer::new_with_stats(stats, buffer_settings.clone());

        Ok(Self {
            db,
            bridge_id,
            chains,
            abi_registry,
            foreign_bridge_address,
            home_chain_id,
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

    /// Seeds the `(home_chain_id, 0x00…00)` sentinel `tokens` row this indexer's own
    /// transfers use for native metadata and stats decimals. Lives here, not in
    /// `server::run`, which must not know indexer specifics -- mirrors how
    /// `AmbIndexer::new` gets its DB handle from `stats.interchain_db_arc()`.
    ///
    /// Idempotent (`upsert_token_info`) and never fatal: a write failure is a
    /// `warn`, not a startup abort, the same rationale as
    /// `evm/log_stream_builder.rs::seed_catchup_floor` -- metadata enrichment
    /// must not be able to stop ingestion. See the gotcha in
    /// `.memory-bank/gotchas.md` for what a missing row actually costs
    /// (missing display metadata and possible NULL `stats_asset_edges.decimals`).
    /// Native classification does not depend on this seed succeeding.
    async fn seed_native_sentinel_token(&self) {
        seed_native_sentinel_token_into(&self.db, self.bridge_id, self.home_chain_id).await;
    }

    async fn run(ctx: RunContext) -> Result<()> {
        tracing::info!(
            bridge_id = ctx.bridge_id,
            chain_count = ctx.chains.len(),
            "starting xDai indexer"
        );

        check_source_asset_matches_latest(ctx.bridge_id, &ctx.chains, &ctx.abi_registry).await;

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
        let counterpart_chain_id = resolve_counterpart_chain_id(&ctx.abi_registry, chain_id);
        let counterpart_chain = counterpart_chain_id.and_then(|counterpart_chain_id| {
            ctx.chains
                .iter()
                .find(|chain| chain.chain_id == counterpart_chain_id)
        });

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
                counterpart_chain,
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

/// Resolves `chain_id`'s xDai counterpart through the configured Foreign/Home
/// side map (`AbiRegistry`) rather than a hardcoded `1 <-> 100` pair, so
/// legacy source reconstruction also works for a non-mainnet deployment (e.g.
/// Sepolia/Chiado). A resolve failure -- `chain_id` matching neither
/// configured side, or the other side missing entirely -- is not an error:
/// `None` degrades to the pre-existing "no counterpart" behaviour rather than
/// panicking or propagating.
fn resolve_counterpart_chain_id(abi_registry: &AbiRegistry, chain_id: i64) -> Option<i64> {
    [XDaiSide::Foreign, XDaiSide::Home]
        .into_iter()
        .find(|&side| abi_registry.chain_id_for_side(side).ok() == Some(chain_id))
        .and_then(|side| abi_registry.counterpart_chain_id(side).ok())
}

alloy::sol! {
    #[sol(rpc)]
    interface IXDaiForeignBridge {
        function erc20token() external view returns (address);
    }
}

/// The body of [`XDaiIndexer::seed_native_sentinel_token`], as a free
/// function over the database handle.
///
/// Split out purely for testability: the method needs a fully constructed
/// `XDaiIndexer` (and therefore a `StatsService`), while the behaviour worth
/// pinning — that the row lands with `decimals = 18` and that a second call
/// is a no-op — needs nothing but a database. Without this split, "startup
/// actually creates the row" is only ever verified by inference from the
/// stats test, which inserts the row itself.
async fn seed_native_sentinel_token_into(
    db: &InterchainDatabase,
    bridge_id: i32,
    home_chain_id: i64,
) {
    let seed = tokens::ActiveModel {
        chain_id: ActiveValue::Set(home_chain_id),
        address: ActiveValue::Set(NATIVE_SENTINEL.as_slice().to_vec()),
        r#type: ActiveValue::Set(TokenType::Native),
        symbol: ActiveValue::Set(Some("xDAI".to_string())),
        name: ActiveValue::Set(Some("xDai".to_string())),
        decimals: ActiveValue::Set(Some(18)),
        ..Default::default()
    };

    if let Err(err) = db.upsert_token_info(seed).await {
        tracing::warn!(
            err = ?err,
            bridge_id,
            chain_id = home_chain_id,
            "failed to seed native xDAI metadata; native classification is preserved, \
             but name and decimals will be unavailable until the next successful seed"
        );
    }
}

/// One-off startup sanity check, not a per-block probe: reads the Foreign
/// proxy's `erc20token()` at `latest` and compares it against the *newest*
/// configured version's `source_asset`.
///
/// A mismatch means the bridge was upgraded without a matching
/// `bridges.json` update, so every Ethereum→Gnosis deposit indexed from that
/// point carries the wrong `token_src_address`. Reported at `error` and via
/// [`metrics::XDAI_SOURCE_ASSET_MISMATCH`], **not** at `warn`: the asset
/// comes from a static table rather than from a log, so nothing downstream
/// can notice the divergence and this is the entire detection path.
///
/// Deliberately **not** fatal, and the two artifacts disagreed on this until
/// it was resolved in favour of non-fatal (see `implementation-plan-2.md`
/// "Source-asset validation"): `spawn_configured_indexers` propagates a
/// construction error, so failing here would take the AMB and Avalanche
/// indexers down too — a service-wide outage traded for a labelling error
/// confined to one direction of one bridge, in a condition only reachable
/// after a coordinated proxy upgrade that is itself an operational event.
///
/// Only the newest window is checked: historical windows are immutable and
/// were verified by bisection, and reading `erc20token()` at `latest` says
/// nothing about what it returned at a historical block.
async fn check_source_asset_matches_latest(
    bridge_id: i32,
    chains: &[XDaiChainConfig],
    abi_registry: &AbiRegistry,
) {
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
    let expected = match grammar_for(chain.chain_id, XDaiSide::Foreign, newest.version) {
        Ok(grammar) => grammar.source_asset,
        Err(err) => {
            tracing::warn!(err = ?err, chain_id = chain.chain_id, version = newest.version, "no xDai grammar for configured Foreign version; skipping source-asset sanity check");
            return;
        }
    };
    let Some(expected) = expected else {
        return;
    };

    let contract = IXDaiForeignBridge::new(newest.address, chain.provider.clone());
    match contract.erc20token().call().await {
        Ok(found) if found == expected => {
            metrics::XDAI_SOURCE_ASSET_MISMATCH
                .with_label_values(&[&bridge_id.to_string()])
                .set(0);
        }
        Ok(found) => {
            metrics::XDAI_SOURCE_ASSET_MISMATCH
                .with_label_values(&[&bridge_id.to_string()])
                .set(1);
            tracing::error!(
                bridge_id,
                chain_id = chain.chain_id,
                address = %newest.address,
                version = newest.version,
                expected = %expected,
                found = %found,
                "xDai Foreign proxy erc20token() does not match the configured newest \
                 source_asset: the bridge was upgraded without a matching bridges.json update, \
                 so every Ethereum->Gnosis deposit indexed from now on will carry the wrong \
                 token_src_address. Indexing continues deliberately (a hard failure here would \
                 stop every other bridge too) -- add the new version window to bridges.json and \
                 reindex the affected range"
            );
        }
        // Left unset rather than zeroed: a transient RPC failure is not
        // evidence that the config agrees with the chain.
        Err(err) => tracing::warn!(
            err = ?err,
            bridge_id,
            chain_id = chain.chain_id,
            address = %newest.address,
            "failed to sanity-check xDai Foreign proxy erc20token() against the configured \
             source_asset; the newest source_asset window is unverified this run"
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
                start_block: super::super::version::ETHEREUM_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: foreign_addr,
                    version: 9,
                    started_at_block: super::super::version::ETHEREUM_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(foreign_abi()).unwrap()),
                }],
            },
            XDaiChainConfig {
                chain_id: GNO,
                provider: dummy_provider(),
                start_block: super::super::version::GNOSIS_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: home_addr,
                    version: 7,
                    started_at_block: super::super::version::GNOSIS_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(home_abi()).unwrap()),
                }],
            },
        ];
        AbiRegistry::from_chains(&chains).expect("test registry builds")
    }

    const SEPOLIA: i64 = 11_155_111;
    const CHIADO: i64 = 10_200;

    /// The Sepolia/Chiado testnet pair, ABIs verbatim from `test_registry`'s
    /// mainnet ones (byte-identical `topic0`s on testnet, per
    /// `.memory-bank/research/xdai-bridge-sepolia-chiado-upgrade-history.md`),
    /// only the chain ids and proxy `version`s differ.
    fn sepolia_chiado_registry(foreign_addr: Address, home_addr: Address) -> AbiRegistry {
        let chains = vec![
            XDaiChainConfig {
                chain_id: SEPOLIA,
                provider: dummy_provider(),
                start_block: super::super::version::SEPOLIA_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: foreign_addr,
                    version: 2,
                    started_at_block: super::super::version::SEPOLIA_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(foreign_abi()).unwrap()),
                }],
            },
            XDaiChainConfig {
                chain_id: CHIADO,
                provider: dummy_provider(),
                start_block: super::super::version::CHIADO_EPOCH_FLOOR_BLOCK,
                contracts: vec![XDaiContractConfig {
                    address: home_addr,
                    version: 3,
                    started_at_block: super::super::version::CHIADO_EPOCH_FLOOR_BLOCK,
                    abi: Some(serde_json::to_value(home_abi()).unwrap()),
                }],
            },
        ];
        AbiRegistry::from_chains(&chains).expect("Sepolia/Chiado test registry builds")
    }

    /// Acceptance criterion 18: counterpart resolution goes through the
    /// configured Foreign/Home side map, not a hardcoded `1 <-> 100` pair --
    /// and therefore resolves correctly for a non-mainnet pair too.
    #[test]
    fn resolve_counterpart_chain_id_works_for_mainnet_and_sepolia_chiado() {
        let mainnet_registry = test_registry(
            address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016"),
            address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6"),
        );
        assert_eq!(
            resolve_counterpart_chain_id(&mainnet_registry, ETH),
            Some(GNO)
        );
        assert_eq!(
            resolve_counterpart_chain_id(&mainnet_registry, GNO),
            Some(ETH)
        );
        assert_eq!(
            resolve_counterpart_chain_id(&mainnet_registry, 999_999),
            None,
            "a chain matching neither configured side must resolve to None, not panic"
        );

        let testnet_registry =
            sepolia_chiado_registry(Address::repeat_byte(0xAA), Address::repeat_byte(0xBB));
        assert_eq!(
            resolve_counterpart_chain_id(&testnet_registry, SEPOLIA),
            Some(CHIADO),
            "Sepolia's counterpart must resolve to Chiado, not mainnet Gnosis (100)"
        );
        assert_eq!(
            resolve_counterpart_chain_id(&testnet_registry, CHIADO),
            Some(SEPOLIA),
            "Chiado's counterpart must resolve to Sepolia, not mainnet Ethereum (1)"
        );
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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

    /// The startup seed itself, not its consequences. The stats test below
    /// inserts the sentinel row by hand and then proves what having it buys;
    /// this proves the indexer's own startup path actually creates it, with
    /// `decimals = 18` — the value `stats_asset_edges` reads — and that a
    /// restart is a no-op rather than a conflict.
    ///
    /// Native identity remains known without the seed, but its human-readable
    /// metadata and edge decimals must come from this row (never ERC-20 RPC).
    #[tokio::test]
    #[ignore = "needs database to run"]
    async fn seed_native_sentinel_token_creates_the_row_and_is_idempotent() {
        let db = init_db("xdai_seed_native_sentinel_token").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        seed_native_sentinel_token_into(&interchain_db, BRIDGE_ID, GNO).await;

        let row = interchain_db
            .get_token_info(GNO as u64, NATIVE_SENTINEL.as_slice().to_vec())
            .await
            .expect("token lookup succeeds")
            .expect("the sentinel row must exist after the seed");

        assert_eq!(row.r#type, TokenType::Native);
        assert_eq!(row.decimals, Some(18), "stats_asset_edges reads this");
        assert_eq!(row.symbol.as_deref(), Some("xDAI"));
        assert_eq!(row.name.as_deref(), Some("xDai"));

        // A restart must not conflict, and must not degrade the row.
        seed_native_sentinel_token_into(&interchain_db, BRIDGE_ID, GNO).await;

        let again = interchain_db
            .get_token_info(GNO as u64, NATIVE_SENTINEL.as_slice().to_vec())
            .await
            .expect("token lookup succeeds")
            .expect("the sentinel row must survive a second seed");
        assert_eq!(again.r#type, TokenType::Native);
        assert_eq!(again.decimals, Some(18));
        assert_eq!(again.symbol.as_deref(), Some("xDAI"));
    }

    fn dai_address() -> Address {
        address!("6B175474E89094C44Da98b954EedeAC495271d0F")
    }

    /// Proves the native sentinel actually clears
    /// `transfer_identity_ready_condition` instead of deferring as
    /// `identity_incomplete`, AND that xDai's `conversion` linkage keeps its
    /// two endpoints as two separate assets (not merged, unlike the old
    /// single-asset union-find): a completed Gno→Eth transfer reaches
    /// `stats_processed = 1`, its `src_stats_asset_id` (Gnosis native
    /// sentinel) and `dst_stats_asset_id` (Ethereum DAI) resolve to two
    /// *different* `stats_assets` rows joined by one cross-asset
    /// `stats_asset_edges` row, and that edge's `decimals` comes from the
    /// seeded sentinel row rather than ending up NULL.
    ///
    /// This is the exact behaviour ADR-011 (cross-asset edges and
    /// per-transfer linkage) exists to change: before that work, DAI and
    /// native xDAI were incorrectly folded into one shared `stats_assets`
    /// row by the mirror-only union-find. See
    /// `completed_transfer_reaches_one_shared_stats_asset_with_sentinel_decimals`
    /// in git history for the prior (superseded) contract.
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
    async fn completed_transfer_reaches_two_assets_joined_by_a_conversion_edge() {
        let db = init_db("xdai_stats_projection_sentinel").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        // Mirrors `XDaiIndexer::seed_native_sentinel_token`.
        interchain_db
            .upsert_token_info(interchain_indexer_entity::tokens::ActiveModel {
                chain_id: Set(GNO),
                address: Set(NATIVE_SENTINEL.as_slice().to_vec()),
                r#type: Set(TokenType::Native),
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
            counterpart_chain: None,
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
            counterpart_chain: None,
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
        assert_eq!(
            projected.asset_linkage,
            Some(interchain_indexer_entity::sea_orm_active_enums::TransferAssetLinkage::Conversion)
        );
        let src_asset_id = projected
            .src_stats_asset_id
            .expect("source (Gnosis sentinel) identity must resolve, not defer");
        let dst_asset_id = projected
            .dst_stats_asset_id
            .expect("destination (Ethereum DAI) identity must resolve, not defer");
        assert_ne!(
            src_asset_id, dst_asset_id,
            "a conversion transfer's two endpoints must resolve to two different assets, \
             never merged by the mirror union-find"
        );

        let src_asset_tokens: Vec<(i64, Vec<u8>)> =
            interchain_indexer_entity::stats_asset_tokens::Entity::find()
                .filter(
                    interchain_indexer_entity::stats_asset_tokens::Column::StatsAssetId
                        .eq(src_asset_id),
                )
                .all(conn)
                .await
                .unwrap()
                .into_iter()
                .map(|row| (row.chain_id, row.token_address))
                .collect();
        assert_eq!(
            src_asset_tokens,
            vec![(GNO, NATIVE_SENTINEL.as_slice().to_vec())],
            "the sentinel endpoint must be linked into its own asset, alone: {src_asset_tokens:?}"
        );

        let dst_asset_tokens: Vec<(i64, Vec<u8>)> =
            interchain_indexer_entity::stats_asset_tokens::Entity::find()
                .filter(
                    interchain_indexer_entity::stats_asset_tokens::Column::StatsAssetId
                        .eq(dst_asset_id),
                )
                .all(conn)
                .await
                .unwrap()
                .into_iter()
                .map(|row| (row.chain_id, row.token_address))
                .collect();
        assert_eq!(
            dst_asset_tokens,
            vec![(ETH, dai_address().as_slice().to_vec())],
            "the Ethereum DAI endpoint must be linked into its own asset, alone: {dst_asset_tokens:?}"
        );

        let edge = interchain_indexer_entity::stats_asset_edges::Entity::find_by_id((
            GNO,
            ETH,
            BRIDGE_ID,
            src_asset_id,
            dst_asset_id,
        ))
        .one(conn)
        .await
        .unwrap()
        .expect("cross-asset edge row must exist, joining the two assets");
        assert_eq!(
            edge.decimals,
            Some(18),
            "decimals must come from the seeded sentinel row, not end up NULL"
        );
        assert_eq!(edge.transfers_count, 1);
        assert_eq!(
            edge.cumulative_amount,
            sea_orm::prelude::BigDecimal::from(9_000u64)
        );
    }

    // --- xDai canonical-identity-from-receipt and multiple-execution
    // anomalies (xdai-alias-completion-anomalies) ---
    //
    // The verified on-chain Chiado/Sepolia fixture (`handoff.md`) records
    // every relevant value only truncated (addresses, tx hashes), the same
    // constraint the module doc above notes for the trace-replay tests. These
    // reuse that same convention: every value the fixture gives in full
    // (nonce, block numbers, amounts) is exact; addresses and tx hashes are
    // synthetic placeholders.

    /// Builds a mocked `DynProvider` that answers exactly one
    /// `eth_getTransactionReceipt` + `eth_getBlockByNumber` round trip for
    /// `tx_src`, with a receipt containing the modern (nonce-carrying)
    /// `UserRequestForAffirmation` log. Mirrors
    /// `events.rs::reconstructed_source_uses_counterpart_provider_receipt_and_block`'s
    /// fixture shape, reused here to drive the full indexer pipeline instead
    /// of `fetch_reconstructed_source` directly.
    #[allow(clippy::too_many_arguments)]
    fn push_modern_affirmation_receipt(
        asserter: &alloy::transports::mock::Asserter,
        foreign_addr: Address,
        recipient: Address,
        value: U256,
        nonce: U256,
        tx_src: B256,
        src_block: u64,
        src_timestamp: u64,
        sender: Address,
    ) {
        let foreign_event = event_of(&foreign_abi(), "UserRequestForAffirmation");
        let modern_log = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            src_block,
        );
        let receipt: alloy::rpc::types::TransactionReceipt =
            serde_json::from_value(serde_json::json!({
                "type": "0x2",
                "status": "0x1",
                "cumulativeGasUsed": "0x1",
                "logsBloom": format!("0x{}", "00".repeat(256)),
                "logs": [modern_log],
                "transactionHash": tx_src,
                "transactionIndex": "0x0",
                "blockHash": B256::repeat_byte(9),
                "blockNumber": format!("{src_block:#x}"),
                "gasUsed": "0x1",
                "effectiveGasPrice": "0x1",
                "from": sender,
                "to": foreign_addr,
                "contractAddress": null
            }))
            .unwrap();
        asserter.push_success(&Some(receipt));

        let mut block: alloy::rpc::types::Block = Default::default();
        block.header.inner.number = src_block;
        block.header.inner.timestamp = src_timestamp;
        asserter.push_success(&Some(block));
    }

    fn foreign_chain_config_with_provider(
        foreign_addr: Address,
        provider: DynProvider<Ethereum>,
    ) -> XDaiChainConfig {
        XDaiChainConfig {
            chain_id: ETH,
            provider,
            start_block: super::super::version::ETHEREUM_EPOCH_FLOOR_BLOCK,
            contracts: vec![XDaiContractConfig {
                address: foreign_addr,
                version: 9,
                started_at_block: super::super::version::ETHEREUM_EPOCH_FLOOR_BLOCK,
                abi: None,
            }],
        }
    }

    /// The receipt-derived (hash-alias) and stream-derived (real
    /// `UserRequestForAffirmation`) builds of the *same* source transaction
    /// must be indistinguishable: one canonical nonce-keyed message,
    /// regardless of which side is processed first. This is order (a):
    /// stream arrives first, then the hash-keyed completion whose receipt
    /// resolves to the same nonce.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn hash_alias_after_stream_produces_one_nonce_keyed_message_matching_stream_derived() {
        let db = init_db("xdai_hash_alias_after_stream_nonce_parity").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let foreign_event = event_of(&foreign_abi(), "UserRequestForAffirmation");
        let completed_event = event_of(&home_abi(), "AffirmationCompleted");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x9911_u64);
        let value = U256::from(4_500u64);
        let recipient = Address::repeat_byte(0xC7);
        let sender = Address::repeat_byte(0x51);
        const SRC_BLOCK: u64 = 25_852_059;
        const SRC_TIMESTAMP: u64 = 1_700_000_000;
        let tx_src = B256::repeat_byte(0x61);
        let tx_dst = B256::repeat_byte(0x62);

        let asserter = alloy::transports::mock::Asserter::new();
        let mocked_provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();
        push_modern_affirmation_receipt(
            &asserter,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            SRC_BLOCK,
            SRC_TIMESTAMP,
            sender,
        );
        let counterpart = foreign_chain_config_with_provider(foreign_addr, mocked_provider);

        // (a) The real stream event arrives first.
        let source_log = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            SRC_BLOCK,
        );
        let src_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: SRC_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: None,
        };
        events::dispatch_transaction(
            &src_ctx,
            &[source_log],
            &block_with_timestamp(SRC_TIMESTAMP),
            sender,
        )
        .await
        .expect("stream source dispatch succeeds");

        // Then the hash-keyed completion whose receipt reconstructs to the
        // same nonce.
        let hash_as_u256 = U256::from_be_bytes(tx_src.0);
        let completed_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst,
            47_950_000,
            0,
        );
        let dst_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: 47_950_000,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: Some(&counterpart),
        };
        events::dispatch_transaction(
            &dst_ctx,
            &[completed_log],
            &block_with_timestamp(1_700_100_000),
            Address::ZERO,
        )
        .await
        .expect("hash-keyed completion dispatch succeeds");

        assert!(
            asserter.read_q().is_empty(),
            "exactly the pushed receipt+block must be consumed"
        );

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(ETH, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("exactly one nonce-keyed message row must exist");
        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(
            message.native_id,
            Some(native_id.to_vec()),
            "canonical identity must be the nonce, not the raw destination hash"
        );
        assert_eq!(message.dst_tx_hash, Some(tx_dst.as_slice().to_vec()));
        assert_eq!(message.src_tx_hash, Some(tx_src.as_slice().to_vec()));
        assert_eq!(message.sender_address, Some(sender.as_slice().to_vec()));
        assert_eq!(
            message.protocol_metadata, None,
            "a single execution is not an anomaly"
        );

        let transfer = crosschain_transfers::Entity::find()
            .filter(crosschain_transfers::Column::MessageId.eq(key.message_id))
            .filter(crosschain_transfers::Column::BridgeId.eq(BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("exactly one transfer row must exist");
        assert_eq!(
            transfer.token_src_address,
            Some(dai_address().as_slice().to_vec()),
            "source_asset must resolve through the grammar window, not a guessed fallback"
        );
        assert_eq!(
            transfer.src_amount,
            Some(sea_orm::prelude::BigDecimal::from(4_500u64))
        );
        assert_eq!(
            transfer.dst_amount,
            Some(sea_orm::prelude::BigDecimal::from(4_500u64))
        );
    }

    /// Order (b): the mirror image of the test above -- the hash-keyed
    /// completion is processed **first**, before the real stream event for
    /// the same source transaction has been seen at all. Must reach an
    /// identical final row: no `ensure_identity_and_direction` conflict, no
    /// second transfer.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn hash_alias_before_stream_produces_one_nonce_keyed_message_matching_stream_derived() {
        let db = init_db("xdai_hash_alias_before_stream_nonce_parity").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let foreign_event = event_of(&foreign_abi(), "UserRequestForAffirmation");
        let completed_event = event_of(&home_abi(), "AffirmationCompleted");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x9922_u64);
        let value = U256::from(6_600u64);
        let recipient = Address::repeat_byte(0xC8);
        let sender = Address::repeat_byte(0x52);
        const SRC_BLOCK: u64 = 25_852_059;
        const SRC_TIMESTAMP: u64 = 1_700_000_000;
        let tx_src = B256::repeat_byte(0x71);
        let tx_dst = B256::repeat_byte(0x72);

        let asserter = alloy::transports::mock::Asserter::new();
        let mocked_provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();
        push_modern_affirmation_receipt(
            &asserter,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            SRC_BLOCK,
            SRC_TIMESTAMP,
            sender,
        );
        let counterpart = foreign_chain_config_with_provider(foreign_addr, mocked_provider);

        // (b) The hash-keyed completion arrives first.
        let hash_as_u256 = U256::from_be_bytes(tx_src.0);
        let completed_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst,
            47_950_000,
            0,
        );
        let dst_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: 47_950_000,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: Some(&counterpart),
        };
        events::dispatch_transaction(
            &dst_ctx,
            &[completed_log],
            &block_with_timestamp(1_700_100_000),
            Address::ZERO,
        )
        .await
        .expect("hash-keyed completion dispatch succeeds");

        // Then the real stream event for the same source transaction.
        let source_log = user_request_for_affirmation_log(
            &foreign_event,
            foreign_addr,
            recipient,
            value,
            nonce,
            tx_src,
            SRC_BLOCK,
        );
        let src_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: ETH,
            block_number: SRC_BLOCK,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: None,
        };
        events::dispatch_transaction(
            &src_ctx,
            &[source_log],
            &block_with_timestamp(SRC_TIMESTAMP),
            sender,
        )
        .await
        .expect(
            "stream source dispatch must not conflict with the already-buffered receipt-derived \
             identity",
        );

        assert!(asserter.read_q().is_empty());

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(ETH, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("exactly one nonce-keyed message row must exist");
        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(message.native_id, Some(native_id.to_vec()));
        assert_eq!(message.dst_tx_hash, Some(tx_dst.as_slice().to_vec()));
        assert_eq!(message.src_tx_hash, Some(tx_src.as_slice().to_vec()));
        assert_eq!(message.protocol_metadata, None);

        let transfer_count = crosschain_transfers::Entity::find()
            .filter(crosschain_transfers::Column::MessageId.eq(key.message_id))
            .filter(crosschain_transfers::Column::BridgeId.eq(BRIDGE_ID))
            .all(interchain_db.db.as_ref())
            .await
            .unwrap()
            .len();
        assert_eq!(
            transfer_count, 1,
            "both orders must produce exactly one transfer"
        );
    }

    /// Two destination executions that both reconstruct, via receipt, to the
    /// same canonical nonce (the "double execution" incident this task
    /// exists for): the first processed becomes canonical, the second
    /// becomes exactly one `amb_message_anomalies` row and one
    /// `multiple_executions` entry naming its own transaction -- the
    /// canonical row's own `dst_tx_hash` is never duplicated into the array.
    #[tokio::test]
    #[ignore = "needs database"]
    async fn two_hash_aliases_of_one_nonce_produce_one_canonical_message_and_one_anomaly() {
        let db = init_db("xdai_two_hash_aliases_multiple_execution_anomaly").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let completed_event = event_of(&home_abi(), "AffirmationCompleted");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x9933_u64);
        let value = U256::from(7_700u64);
        let recipient = Address::repeat_byte(0xC9);
        let sender = Address::repeat_byte(0x53);
        const SRC_BLOCK: u64 = 25_852_059;
        const SRC_TIMESTAMP: u64 = 1_700_000_000;
        let tx_src = B256::repeat_byte(0x81);
        // Both destination executions alias the *same* source transaction,
        // as in the real Chiado incident (block 16803580's two aliases).
        let tx_dst_first = B256::repeat_byte(0x82);
        let tx_dst_second = B256::repeat_byte(0x83);

        let asserter = alloy::transports::mock::Asserter::new();
        let mocked_provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();
        // Two `fetch_reconstructed_source` calls, one per destination
        // execution -- push the identical receipt/block pair twice.
        for _ in 0..2 {
            push_modern_affirmation_receipt(
                &asserter,
                foreign_addr,
                recipient,
                value,
                nonce,
                tx_src,
                SRC_BLOCK,
                SRC_TIMESTAMP,
                sender,
            );
        }
        let counterpart = foreign_chain_config_with_provider(foreign_addr, mocked_provider);

        let hash_as_u256 = U256::from_be_bytes(tx_src.0);
        let dst_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: 47_950_100,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: Some(&counterpart),
        };

        let first_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst_first,
            47_950_100,
            0,
        );
        events::dispatch_transaction(
            &dst_ctx,
            &[first_log],
            &block_with_timestamp(1_700_100_000),
            Address::ZERO,
        )
        .await
        .expect("first hash-keyed completion dispatch succeeds");

        let second_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst_second,
            47_950_200,
            0,
        );
        events::dispatch_transaction(
            &dst_ctx,
            &[second_log],
            &block_with_timestamp(1_700_200_000),
            Address::ZERO,
        )
        .await
        .expect("second hash-keyed completion dispatch succeeds");

        assert!(asserter.read_q().is_empty());

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(ETH, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("exactly one canonical nonce-keyed message row must exist");
        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(
            message.dst_tx_hash,
            Some(tx_dst_first.as_slice().to_vec()),
            "the first-processed execution stays canonical"
        );

        let anomalies = interchain_indexer_entity::amb_message_anomalies::Entity::find()
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BridgeId.eq(BRIDGE_ID),
            )
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BufferKey
                    .eq(key.message_id),
            )
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(
            anomalies.len(),
            1,
            "exactly one anomaly row for the second execution"
        );
        assert_eq!(anomalies[0].tx_hash, tx_dst_second.as_slice().to_vec());
        assert_eq!(
            anomalies[0].conflict_tx_hash,
            Some(tx_dst_first.as_slice().to_vec())
        );

        let metadata =
            crate::protocol_metadata::ProtocolMetadata::from_json_value(message.protocol_metadata)
                .expect("protocol_metadata must be populated")
                .multiple_executions
                .expect("multiple_executions namespace must be present");
        assert_eq!(metadata.additional_executions.len(), 1);
        assert_eq!(
            metadata.additional_executions[0].transaction_hash,
            alloy::hex::encode_prefixed(tx_dst_second.as_slice())
        );
    }

    /// Mirror of the test above with the two destination executions
    /// dispatched in the opposite order. AC 12 requires both orders: catch-up
    /// scans blocks backward, so the later-block execution can legitimately
    /// be processed *before* the earlier-block one, and "first" must mean
    /// "first in processing order," not "earliest on chain."
    #[tokio::test]
    #[ignore = "needs database"]
    async fn two_hash_aliases_of_one_nonce_in_reverse_processing_order_produce_one_canonical_message_and_one_anomaly()
     {
        let db = init_db("xdai_two_hash_aliases_multiple_execution_anomaly_reverse_order").await;
        let interchain_db = InterchainDatabase::new(db.client());
        seed_bridge_and_chains(&interchain_db).await;

        let foreign_addr = address!("4aa42145Aa6Ebf72e164C9bBC74fbD3788045016");
        let home_addr = address!("7301CFA0e1756B71869E93d4e4Dca5c7d0eb0AA6");
        let registry = test_registry(foreign_addr, home_addr);
        let message_hash_lookup: Arc<DashMap<B256, Key>> = Arc::new(DashMap::new());
        let pending_message_hash_events: Arc<DashMap<B256, PendingMessageHashEvents>> =
            Arc::new(DashMap::new());
        let completed_event = event_of(&home_abi(), "AffirmationCompleted");

        let buffer = MessageBuffer::<Message>::new(
            interchain_db.clone(),
            MessageBufferSettings {
                hot_ttl: Duration::from_secs(60),
                maintenance_interval: Duration::from_secs(60),
            },
        );

        let nonce = U256::from(0x9933_u64);
        let value = U256::from(7_700u64);
        let recipient = Address::repeat_byte(0xC9);
        let sender = Address::repeat_byte(0x53);
        const SRC_BLOCK: u64 = 25_852_059;
        const SRC_TIMESTAMP: u64 = 1_700_000_000;
        let tx_src = B256::repeat_byte(0x81);
        // Same two aliases of the same source transaction as the
        // forward-order test above, dispatched in the opposite order.
        let tx_dst_first = B256::repeat_byte(0x82);
        let tx_dst_second = B256::repeat_byte(0x83);

        let asserter = alloy::transports::mock::Asserter::new();
        let mocked_provider = ProviderBuilder::new()
            .connect_mocked_client(asserter.clone())
            .erased();
        // Two `fetch_reconstructed_source` calls, one per destination
        // execution -- push the identical receipt/block pair twice.
        for _ in 0..2 {
            push_modern_affirmation_receipt(
                &asserter,
                foreign_addr,
                recipient,
                value,
                nonce,
                tx_src,
                SRC_BLOCK,
                SRC_TIMESTAMP,
                sender,
            );
        }
        let counterpart = foreign_chain_config_with_provider(foreign_addr, mocked_provider);

        let hash_as_u256 = U256::from_be_bytes(tx_src.0);
        let dst_ctx = EventContext {
            bridge_id: BRIDGE_ID,
            chain_id: GNO,
            block_number: 47_950_100,
            abi_registry: &registry,
            buffer: &buffer,
            foreign_bridge_address: foreign_addr,
            message_hash_lookup: &message_hash_lookup,
            pending_message_hash_events: &pending_message_hash_events,
            counterpart_chain: Some(&counterpart),
        };

        let second_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst_second,
            47_950_200,
            0,
        );
        // Dispatched *first* this time -- it is the later block, exercising
        // the backward catch-up processing order.
        events::dispatch_transaction(
            &dst_ctx,
            &[second_log],
            &block_with_timestamp(1_700_200_000),
            Address::ZERO,
        )
        .await
        .expect("first-processed (later-block) hash-keyed completion dispatch succeeds");

        let first_log = affirmation_completed_log(
            &completed_event,
            home_addr,
            recipient,
            value,
            hash_as_u256,
            tx_dst_first,
            47_950_100,
            0,
        );
        events::dispatch_transaction(
            &dst_ctx,
            &[first_log],
            &block_with_timestamp(1_700_100_000),
            Address::ZERO,
        )
        .await
        .expect("second-processed (earlier-block) hash-keyed completion dispatch succeeds");

        assert!(asserter.read_q().is_empty());

        buffer.run().await.expect("maintenance flush succeeds");

        let native_id = native_id_blob(ETH, nonce).unwrap();
        let key = key_from_native_id(&native_id, BRIDGE_ID).unwrap();

        let message = crosschain_messages::Entity::find_by_id((key.message_id, BRIDGE_ID))
            .one(interchain_db.db.as_ref())
            .await
            .unwrap()
            .expect("exactly one canonical nonce-keyed message row must exist");
        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(
            message.dst_tx_hash,
            Some(tx_dst_second.as_slice().to_vec()),
            "the first-*processed* execution stays canonical, even though it \
             is the later block"
        );

        let anomalies = interchain_indexer_entity::amb_message_anomalies::Entity::find()
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BridgeId.eq(BRIDGE_ID),
            )
            .filter(
                interchain_indexer_entity::amb_message_anomalies::Column::BufferKey
                    .eq(key.message_id),
            )
            .all(interchain_db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(
            anomalies.len(),
            1,
            "exactly one anomaly row for the second-processed execution"
        );
        assert_eq!(anomalies[0].tx_hash, tx_dst_first.as_slice().to_vec());
        assert_eq!(
            anomalies[0].conflict_tx_hash,
            Some(tx_dst_second.as_slice().to_vec())
        );

        let metadata =
            crate::protocol_metadata::ProtocolMetadata::from_json_value(message.protocol_metadata)
                .expect("protocol_metadata must be populated")
                .multiple_executions
                .expect("multiple_executions namespace must be present");
        assert_eq!(metadata.additional_executions.len(), 1);
        assert_eq!(
            metadata.additional_executions[0].transaction_hash,
            alloy::hex::encode_prefixed(tx_dst_first.as_slice())
        );
    }
}
