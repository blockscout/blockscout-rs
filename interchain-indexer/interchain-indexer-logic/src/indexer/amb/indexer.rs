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
    rpc::types::Log,
};
use anyhow::{Result, ensure};
use dashmap::DashMap;
use futures::{StreamExt, stream::SelectAll};
use serde_json::Value;
use tokio::task::JoinHandle;
use tonic::async_trait;

use crate::{
    CrosschainIndexer, CrosschainIndexerState, CrosschainIndexerStatus, InterchainDatabase,
    MessageBufferSettings, StatsService,
    message_buffer::{Key, MessageBuffer},
};

use super::{
    abi::AbiRegistry,
    events::{self, EventContext, PendingMessageHashEvents},
    settings::AmbIndexerSettings,
    types::Message,
};

#[derive(Clone)]
pub struct AmbChainConfig {
    pub chain_id: i64,
    pub provider: DynProvider<Ethereum>,
    pub amb_proxy_address: Address,
    pub mediator_address: Address,
    pub start_block: u64,
    pub amb_version: i16,
    pub mediator_version: i16,
    pub amb_abi: Option<Value>,
    pub mediator_abi: Option<Value>,
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

    async fn run(ctx: Arc<RunContext>) -> Result<()> {
        tracing::info!(
            bridge_id = ctx.bridge_id,
            chain_count = ctx.chains.len(),
            "starting AMB indexer"
        );

        let mut combined_stream = SelectAll::new();
        for chain in &ctx.chains {
            let filter = ctx.abi_registry.filter_for_chain(chain.chain_id)?;
            let stream = crate::indexer::evm::build_log_stream_for_chain(
                chain.provider.clone(),
                chain.chain_id,
                ctx.bridge_id,
                filter,
                chain.start_block,
                &ctx.db,
                ctx.settings.pull_interval_ms,
                ctx.settings.batch_size,
            )
            .await?;
            combined_stream.push(stream);
        }

        while let Some((chain_id, provider, batch)) = combined_stream.next().await {
            if batch.is_empty() {
                continue;
            }
            if let Err(err) = Self::process_batch(&ctx, chain_id, &provider, &batch).await {
                tracing::error!(
                    err = ?err,
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    batch_size = batch.len(),
                    "failed to process AMB log batch, continuing"
                );
            }
        }

        Ok(())
    }

    async fn process_batch(
        ctx: &RunContext,
        chain_id: i64,
        provider: &DynProvider<Ethereum>,
        batch: &[Log],
    ) -> Result<()> {
        let logs_by_tx = crate::indexer::evm::group_logs_by_transaction(batch);
        let hashes = logs_by_tx.keys().copied().collect::<Vec<_>>();
        let receipts = crate::indexer::evm::fetch_receipts_for_transactions(
            provider,
            hashes,
            ctx.settings.receipt_concurrency as usize,
        )
        .await?;

        for (hash, _logs) in logs_by_tx {
            let Some(receipt) = receipts.get(&hash) else {
                tracing::warn!(
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    "missing AMB receipt fetched for transaction"
                );
                continue;
            };
            let event_ctx = EventContext {
                bridge_id: ctx.bridge_id,
                chain_id,
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
                tracing::error!(
                    err = ?err,
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    "failed to dispatch AMB transaction, continuing"
                );
            }
        }

        Ok(())
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

        let run_ctx = Arc::new(self.run_context());
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
                tracing::error!(err = ?err, bridge_id, "AMB indexer task stopped with error");
                *state.write() = CrosschainIndexerState::Failed(format!("{err:#}"));
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
            .map(|chain| chain.mediator_version)
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
