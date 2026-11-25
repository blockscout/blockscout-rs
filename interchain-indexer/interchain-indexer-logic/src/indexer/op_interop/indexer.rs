// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

use alloy::{
    network::Ethereum,
    primitives::{Address, B256, ChainId},
    providers::DynProvider,
    rpc::types::{Filter, Log},
};
use anyhow::{Context, Result, ensure};
use interchain_indexer_entity::{sea_orm_active_enums::TokenType, tokens};
use sea_orm::ActiveValue;
use tokio::task::JoinHandle;
use tonic::async_trait;

use crate::{
    CrosschainIndexer, CrosschainIndexerState, CrosschainIndexerStatus, InterchainDatabase,
    MessageBufferSettings, StatsService,
    indexer::{
        failure_ledger::{BlockRange, FailureLedger},
        range_driver::{BatchError, RangeDriver, RangeProcessor},
        xdai::NATIVE_SENTINEL,
    },
    log_stream::LogBatch,
    message_buffer::MessageBuffer,
    secret::redact_urls,
};

use super::{
    abi::OpInteropRegistry,
    consolidation,
    events::{self, EventContext, TxPlan},
    settings::OpInteropIndexerSettings,
    types::{ExecutingLeg, Message},
};

/// One configured contract of one chain, valid from `started_at_block`.
#[derive(Clone, Debug)]
pub struct OpInteropContractConfig {
    pub address: Address,
    /// Raw `kind` from `bridges.json`; parsed and validated here, not in the server.
    pub kind: Option<String>,
    pub version: i16,
    pub started_at_block: u64,
}

/// One chain this bridge indexes: its provider and its three predeploy
/// entries (`cross_l2_inbox`, `l2_to_l2_messenger`, `superchain_eth_bridge`).
#[derive(Clone)]
pub struct OpInteropChainConfig {
    pub chain_id: i64,
    pub provider: DynProvider<Ethereum>,
    /// Lowest `started_at_block` among `contracts`.
    pub start_block: u64,
    pub contracts: Vec<OpInteropContractConfig>,
}

pub struct OpInteropIndexer {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    chains: Vec<OpInteropChainConfig>,
    registry: Arc<OpInteropRegistry>,
    configured_chains: Arc<HashSet<ChainId>>,
    settings: OpInteropIndexerSettings,
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
    chains: Vec<OpInteropChainConfig>,
    registry: Arc<OpInteropRegistry>,
    configured_chains: Arc<HashSet<ChainId>>,
    settings: OpInteropIndexerSettings,
    buffer: Arc<MessageBuffer<Message>>,
}

impl OpInteropIndexer {
    pub fn new(
        stats: Arc<StatsService>,
        bridge_id: i32,
        chains: Vec<OpInteropChainConfig>,
        settings: &OpInteropIndexerSettings,
        buffer_settings: &MessageBufferSettings,
    ) -> Result<Self> {
        ensure!(
            !chains.is_empty(),
            "OP interop indexer requires at least one chain"
        );
        ensure!(
            settings.batch_size >= 1,
            "OP interop indexer batch_size must be at least 1"
        );
        settings
            .failure_retry
            .validate()
            .context("invalid OP interop indexer failure_retry settings")?;

        let configured_chains = chains
            .iter()
            .map(|chain| {
                ChainId::try_from(chain.chain_id).with_context(|| {
                    format!(
                        "OP interop bridge {bridge_id} has a negative chain id {}",
                        chain.chain_id
                    )
                })
            })
            .collect::<Result<HashSet<_>>>()?;
        if configured_chains.len() < 2 {
            tracing::warn!(
                bridge_id,
                chains_count = configured_chains.len(),
                "OP interop bridge has fewer than two chains; every message will be skipped"
            );
        }

        let registry = Arc::new(
            OpInteropRegistry::from_chains(&chains)
                .with_context(|| format!("invalid OP interop bridge {bridge_id} config"))?,
        );
        let db = stats.interchain_db_arc();
        let buffer = MessageBuffer::new_with_stats(stats, buffer_settings.clone());

        Ok(Self {
            db,
            bridge_id,
            chains,
            registry,
            configured_chains: Arc::new(configured_chains),
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
            registry: self.registry.clone(),
            configured_chains: self.configured_chains.clone(),
            settings: self.settings.clone(),
            buffer: self.buffer.clone(),
        }
    }

    async fn run(ctx: RunContext) -> Result<()> {
        tracing::info!(
            bridge_id = ctx.bridge_id,
            chain_count = ctx.chains.len(),
            confirmation_block_tag = ctx.settings.confirmation_block_tag.as_str(),
            "starting OP interop indexer"
        );

        let mut streams = Vec::with_capacity(ctx.chains.len());
        for chain in &ctx.chains {
            let chain_id = chain.chain_id;
            let filter = ctx.registry.filter_for_chain(chain_id)?;
            let stream = crate::indexer::evm::build_log_stream_for_chain_with_head(
                chain.provider.clone(),
                chain_id,
                ctx.bridge_id,
                filter,
                chain.start_block,
                &ctx.db,
                ctx.settings.pull_interval_ms,
                ctx.settings.batch_size,
                ctx.settings.confirmation_block_tag.into(),
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

    /// Makes no RPC call: everything comes from the log rows. It makes one
    /// database write per batch that holds executions: every `ExecutingMessage`
    /// is written as an unlinked ledger row before any buffer action of the
    /// batch is applied, and a write failure attributes the executions'
    /// blocks, which then apply nothing to the buffer.
    ///
    /// A log row missing a field this indexer keeps is a processing error
    /// attributed to its block, never a skip: the checkpoint would otherwise
    /// certify a block whose data was lost. No log of a failed block is
    /// dispatched (and none at all when the failing row has no block number),
    /// so the retry replays that block from scratch. Failed transactions' blocks are
    /// returned as single-block ranges, exactly as
    /// `XDaiIndexer::process_batch` does, so the retry path replays only
    /// them.
    async fn process_batch(
        ctx: &RunContext,
        chain_id: i64,
        batch: &[Log],
    ) -> Result<(), BatchError> {
        if batch.is_empty() {
            return Ok(());
        }
        let scanned_chain_id = ChainId::try_from(chain_id)
            .with_context(|| format!("negative OP interop chain id {chain_id}"))?;

        // Validation comes before grouping: `group_logs_by_transaction`
        // silently drops a log without a transaction hash, and the planner
        // relies on every log it sees having been validated.
        let mut failed_blocks: Vec<u64> = Vec::new();
        let mut whole_range_failed = false;
        let mut last_err: Option<anyhow::Error> = None;
        let mut failed_count = 0usize;
        let mut valid: Vec<Log> = Vec::with_capacity(batch.len());

        for log in batch {
            match events::log_ref(scanned_chain_id, log) {
                Ok(_) => valid.push(log.clone()),
                Err(err) => {
                    tracing::warn!(
                        err = ?err,
                        bridge_id = ctx.bridge_id,
                        chain_id,
                        block_number = ?log.block_number,
                        tx_hash = ?log.transaction_hash,
                        log_index = ?log.log_index,
                        "invalid OP interop log row; attributing its block as failed"
                    );
                    match log.block_number {
                        Some(block_number) => failed_blocks.push(block_number),
                        None => whole_range_failed = true,
                    }
                    failed_count += 1;
                    last_err = Some(err);
                }
            }
        }

        // A block with an invalid log is replayed whole by the retry path, so
        // none of its logs are dispatched now: a partially planned
        // transaction could finalize a message as an unpaired relay, and the
        // replay could never restore the lost `executing_message` leg. With a
        // failure that has no block number the whole range is retried, so
        // nothing is dispatched at all.
        if whole_range_failed {
            valid.clear();
        } else if !failed_blocks.is_empty() {
            valid.retain(|log| {
                log.block_number
                    .is_none_or(|number| !failed_blocks.contains(&number))
            });
        }

        let event_ctx = EventContext {
            bridge_id: ctx.bridge_id,
            chain_id: scanned_chain_id,
            registry: &ctx.registry,
            configured_chains: &ctx.configured_chains,
            buffer: &ctx.buffer,
        };

        // Phase 1 for every transaction: pure plans plus their metrics.
        let mut planned: Vec<(B256, Vec<u64>, TxPlan)> = Vec::new();
        for (hash, mut logs) in crate::indexer::evm::group_logs_by_transaction(&valid) {
            // Catch-up batches arrive descending; the relay window needs ascending.
            logs.sort_by_key(|log| log.log_index);
            let blocks: Vec<u64> = logs.iter().filter_map(|log| log.block_number).collect();
            match events::plan_and_report(&event_ctx, &logs) {
                Ok(plan) => planned.push((hash, blocks, plan)),
                Err(err) => {
                    tracing::warn!(
                        err = ?err,
                        bridge_id = ctx.bridge_id,
                        chain_id,
                        tx_hash = %hash,
                        "failed to plan OP interop transaction"
                    );
                    failed_blocks.extend(blocks);
                    failed_count += 1;
                    last_err = Some(err);
                }
            }
        }

        // Raw executions are written before any buffer effect of this batch
        // exists. Maintenance flushes concurrently and can move the checkpoint
        // past a block whose entry it flushed; with this order, every such
        // block already has its raw rows durable, or its failure is on the way
        // to the failure ledger.
        let executions: Vec<&ExecutingLeg> = planned
            .iter()
            .flat_map(|(_, _, plan)| plan.executions.iter())
            .collect();
        let mut unwritten_blocks: BTreeSet<u64> = BTreeSet::new();
        if let Err(err) = write_executions(ctx, &executions).await {
            // Every block of the batch's executions: batched_upsert may have
            // committed some chunks and not others.
            unwritten_blocks = executions
                .iter()
                .map(|exec| exec.log.block_number)
                .collect();
            tracing::warn!(
                err = ?err,
                bridge_id = ctx.bridge_id,
                chain_id,
                executions = executions.len(),
                first_block = ?unwritten_blocks.first(),
                last_block = ?unwritten_blocks.last(),
                "failed to write OP interop raw executions; attributing their blocks as failed"
            );
            failed_blocks.extend(unwritten_blocks.iter().copied());
            failed_count += 1;
            last_err = Some(err);
        }

        // Phase 2: the buffer actions, except in blocks whose raw rows were not
        // written. Those blocks are replayed whole, like a block with an
        // invalid row: applied now, a relay could finalize its message before
        // the replay, and the replay would then put that relay into a fresh
        // entry with no source, which never finalizes.
        for (hash, blocks, plan) in planned {
            if blocks.iter().any(|block| unwritten_blocks.contains(block)) {
                continue;
            }
            if let Err(err) =
                events::apply_actions(&event_ctx, plan.actions, hash, blocks.first().copied()).await
            {
                tracing::warn!(
                    err = ?err,
                    bridge_id = ctx.bridge_id,
                    chain_id,
                    tx_hash = %hash,
                    "failed to apply OP interop transaction"
                );
                failed_blocks.extend(blocks);
                failed_count += 1;
                last_err = Some(err);
            }
        }

        if let Some(err) = last_err {
            failed_blocks.sort_unstable();
            failed_blocks.dedup();
            // Empty means "the whole yielded range" to the driver.
            let attributed = if whole_range_failed {
                vec![]
            } else {
                failed_blocks
                    .into_iter()
                    .map(|number| BlockRange {
                        from: number,
                        to: number,
                    })
                    .collect()
            };

            return Err(BatchError {
                error: err.context(format!(
                    "{failed_count} OP interop log(s) or transaction(s) failed to process"
                )),
                attributed,
            });
        }

        Ok(())
    }
}

/// One unlinked ledger row per execution, in one idempotent call. The flush
/// links the delivering one later; the conflict rule never clears a link.
async fn write_executions(ctx: &RunContext, executions: &[&ExecutingLeg]) -> Result<()> {
    if executions.is_empty() {
        return Ok(());
    }
    let rows = executions
        .iter()
        .map(|exec| {
            consolidation::execution_row(ctx.bridge_id, exec, None).with_context(|| {
                format!(
                    "OP interop execution (block {}, log_index {}) does not fit the storage types",
                    exec.log.block_number, exec.log.log_index
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let count = rows.len();
    crate::message_buffer::upsert_op_interop_legs(ctx.db.db.as_ref(), rows)
        .await
        .with_context(|| format!("failed to write {count} OP interop raw execution row(s)"))
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

    /// The forward filter, never a narrower one: a replay must see every log
    /// the scan saw.
    fn log_filter(&self, chain_id: i64) -> Result<Filter> {
        self.registry.filter_for_chain(chain_id)
    }

    fn batch_size(&self) -> u64 {
        self.settings.batch_size
    }

    async fn process(&self, chain_id: i64, batch: &LogBatch) -> Result<(), BatchError> {
        if batch.logs.is_empty() {
            return Ok(());
        }
        OpInteropIndexer::process_batch(self, chain_id, &batch.logs).await
    }
}

#[async_trait]
impl CrosschainIndexer for OpInteropIndexer {
    fn name(&self) -> String {
        "OP Interop".into()
    }

    fn description(&self) -> String {
        "OP Stack native interop (L2ToL2CrossDomainMessenger) indexer".into()
    }

    async fn start(&self) -> Result<()> {
        if self
            .is_running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            tracing::debug!(
                bridge_id = self.bridge_id,
                "OP interop indexer already running"
            );
            return Ok(());
        }

        *self.state.write() = CrosschainIndexerState::Running;

        let chain_ids: Vec<i64> = self.chains.iter().map(|chain| chain.chain_id).collect();
        seed_native_tokens_into(&self.db, self.bridge_id, &chain_ids).await;

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
                tracing::error!(err = %redacted, bridge_id, "OP interop indexer task stopped with error");
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
                "confirmation_block_tag".to_string(),
                serde_json::json!(self.settings.confirmation_block_tag.as_str()),
            ),
        ]);

        CrosschainIndexerStatus {
            state: self.state.read().clone(),
            init_timestamp: self.init_timestamp,
            extra_info,
        }
    }
}

/// Seeds the `ETH` / 18 native token row of every chain of the bridge, so that
/// stats read decimals for the zero-address sentinel the ETH transfers use.
///
/// Idempotent, and non-fatal: a failure is logged and the next chain is still
/// seeded. The seed overwrites name, symbol and decimals of an existing native
/// row on every start and keeps an existing icon.
async fn seed_native_tokens_into(db: &InterchainDatabase, bridge_id: i32, chain_ids: &[i64]) {
    for &chain_id in chain_ids {
        let seed = tokens::ActiveModel {
            chain_id: ActiveValue::Set(chain_id),
            address: ActiveValue::Set(NATIVE_SENTINEL.as_slice().to_vec()),
            r#type: ActiveValue::Set(TokenType::Native),
            symbol: ActiveValue::Set(Some("ETH".to_string())),
            name: ActiveValue::Set(Some("Ether".to_string())),
            decimals: ActiveValue::Set(Some(18)),
            ..Default::default()
        };

        if let Err(err) = db.upsert_token_info(seed).await {
            tracing::warn!(
                err = ?err,
                bridge_id,
                chain_id,
                "failed to seed the native ETH token of an OP interop chain; the token row \
                 and its decimals will be missing until the next successful seed"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    //! DB-backed replay tests over the simulated devnet fixtures: log batches
    //! go through `process_batch`, one maintenance cycle (`buffer.run()`)
    //! flushes them, and the assertions read the resulting rows.

    use std::time::Duration;

    use alloy::{
        primitives::{B256, LogData, U256, b256},
        rpc::types::Log,
    };
    use interchain_indexer_entity::{
        bridges, chains, crosschain_messages, crosschain_transfers, op_interop_legs,
        pending_messages,
        sea_orm_active_enums::{BridgeType, MessageStatus},
        stats_asset_edges, stats_asset_tokens, stats_messages,
    };
    use pretty_assertions::assert_eq;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter,
        QueryOrder, TransactionTrait,
    };
    use serde_json::json;

    use super::*;
    use crate::{
        IndexedChains,
        indexer::{
            failure_ledger::settings::FailureRetrySettings,
            op_interop::test_fixtures::{
                DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, contracts, foreign_read, pull_read,
                relay_logs, send_calls, sent_message_logs,
            },
        },
        log_stream::{ScanDirection, test_support::HeadMockService},
        test_utils::init_db,
        write_api,
    };

    const BRIDGE_ID: i32 = 2004;
    const SOURCE: i64 = SOURCE_CHAIN_ID as i64;
    const DESTINATION: i64 = DESTINATION_CHAIN_ID as i64;
    const MESSAGE_0_ID_HASH: B256 =
        b256!("274fa1e522d77c94a725679b973bd3070ac910488a6a5cf023c1469f94655eaa");
    const SOURCE_TX: B256 =
        b256!("d7ff0cd43c35c6438f899a68526128c57b92995119a0900ea9493990ea8f5b9c");
    const DESTINATION_TX: B256 =
        b256!("7155e2226f4cc4ffbb8987e756fa5e450a9c40c9e2e0dcd7f1195ae8aa58d03d");
    const SOURCE_BLOCK: u64 = 498_922;

    async fn seed_bridge_and_chains(db: &InterchainDatabase) {
        bridges::Entity::insert(bridges::ActiveModel {
            id: Set(BRIDGE_ID),
            name: Set("OP Interop".to_string()),
            r#type: Set(Some(BridgeType::OpInterop)),
            ..Default::default()
        })
        .exec(db.db.as_ref())
        .await
        .unwrap();
        chains::Entity::insert_many([
            chains::ActiveModel {
                id: Set(SOURCE),
                name: Set("OP Sepolia Devnet 4".to_string()),
                ..Default::default()
            },
            chains::ActiveModel {
                id: Set(DESTINATION),
                name: Set("OP Sepolia Devnet 5".to_string()),
                ..Default::default()
            },
        ])
        .exec(db.db.as_ref())
        .await
        .unwrap();
    }

    fn buffer_settings(hot_ttl: Duration) -> MessageBufferSettings {
        MessageBufferSettings {
            hot_ttl,
            maintenance_interval: Duration::from_secs(60),
        }
    }

    /// A buffer wired to a `StatsService` that indexes both chains, so an
    /// `initiated` message is not countable but a `completed` one is.
    fn new_buffer(db: &InterchainDatabase, hot_ttl: Duration) -> Arc<MessageBuffer<Message>> {
        let stats = Arc::new(StatsService::new(
            Arc::new(db.clone()),
            None,
            Default::default(),
            IndexedChains::from_pairs([(BRIDGE_ID, SOURCE), (BRIDGE_ID, DESTINATION)]),
        ));
        MessageBuffer::new_with_stats(stats, buffer_settings(hot_ttl))
    }

    /// `process_batch` makes no RPC call, so the providers are never used.
    fn run_context(db: &InterchainDatabase, buffer: Arc<MessageBuffer<Message>>) -> RunContext {
        let provider = HeadMockService::new(0, vec![]).provider();
        let chains = vec![
            OpInteropChainConfig {
                chain_id: SOURCE,
                provider: provider.clone(),
                start_block: 0,
                contracts: contracts(0),
            },
            OpInteropChainConfig {
                chain_id: DESTINATION,
                provider,
                start_block: 0,
                contracts: contracts(0),
            },
        ];
        RunContext {
            db: Arc::new(db.clone()),
            bridge_id: BRIDGE_ID,
            registry: Arc::new(OpInteropRegistry::from_chains(&chains).unwrap()),
            configured_chains: Arc::new(HashSet::from([SOURCE_CHAIN_ID, DESTINATION_CHAIN_ID])),
            chains,
            settings: OpInteropIndexerSettings::default(),
            buffer,
        }
    }

    /// The logs of the first call of the send fixture (`sendETH` plus the
    /// message-0 `SentMessage`).
    fn send_batch() -> Vec<Log> {
        send_calls().remove(0)
    }

    async fn feed(ctx: &RunContext, chain_id: i64, logs: Vec<Log>) {
        OpInteropIndexer::process_batch(ctx, chain_id, &logs)
            .await
            .unwrap_or_else(|err| panic!("process_batch failed: {:#}", err.error));
    }

    async fn messages(db: &InterchainDatabase) -> Vec<crosschain_messages::Model> {
        crosschain_messages::Entity::find()
            .all(db.db.as_ref())
            .await
            .unwrap()
    }

    async fn leg_kinds(db: &InterchainDatabase) -> Vec<String> {
        let mut kinds = op_interop_legs::Entity::find()
            .all(db.db.as_ref())
            .await
            .unwrap()
            .into_iter()
            .map(|leg| leg.event_kind)
            .collect::<Vec<_>>();
        kinds.sort();
        kinds
    }

    async fn execution_rows(db: &InterchainDatabase) -> Vec<op_interop_legs::Model> {
        op_interop_legs::Entity::find()
            .filter(op_interop_legs::Column::EventKind.eq("executing_message"))
            .order_by_asc(op_interop_legs::Column::ChainId)
            .order_by_asc(op_interop_legs::Column::BlockNumber)
            .order_by_asc(op_interop_legs::Column::LogIndex)
            .all(db.db.as_ref())
            .await
            .unwrap()
    }

    /// The relay fixture with every log at `log_index >= at` shifted up by
    /// one, plus `extra` (already at `at`).
    fn relay_logs_with(extra: Log, at: u64) -> Vec<Log> {
        let mut logs: Vec<Log> = relay_logs()
            .into_iter()
            .map(|mut log| {
                let index = log.log_index.expect("fixture log has an index");
                if index >= at {
                    log.log_index = Some(index + 1);
                }
                log
            })
            .collect();
        logs.push(extra);
        logs
    }

    async fn pending_count(db: &InterchainDatabase) -> u64 {
        pending_messages::Entity::find()
            .count(db.db.as_ref())
            .await
            .unwrap()
    }

    fn op_interop(message: &crosschain_messages::Model) -> serde_json::Value {
        message.protocol_metadata.clone().expect("metadata is set")["op_interop"].clone()
    }

    fn source_metadata() -> serde_json::Value {
        json!({
            "block_number": SOURCE_BLOCK,
            "block_hash": "0x8580247817bcfdd8f258ebd2f9a73aeddb4b87953114a723da65206fa002c8b5",
            "log_index": 1,
        })
    }

    fn complete_metadata() -> serde_json::Value {
        json!({
            "nonce": "0",
            "source": source_metadata(),
            "destination": {
                "block_number": 895_701,
                "block_hash": "0x7ea9e26557b70150ae7360da608f0f231e30397b4e5b4c98f4edd431d6630c73",
                "log_index": 3,
            },
            "executing_message": {
                "msg_hash": "0xbb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb",
                "log_index": 0,
                "identifier": {
                    "origin": "0x4200000000000000000000000000000000000023",
                    "chain_id": "420130019",
                    "block_number": "498922",
                    "log_index": "1",
                    "timestamp": "1794670098",
                },
            },
        })
    }

    fn assert_completed(message: &crosschain_messages::Model) {
        assert_eq!(message.status, MessageStatus::Completed);
        assert_eq!(message.native_id, Some(MESSAGE_0_ID_HASH.to_vec()));
        assert_eq!(message.src_tx_hash, Some(SOURCE_TX.to_vec()));
        assert_eq!(message.dst_tx_hash, Some(DESTINATION_TX.to_vec()));
        assert!(message.last_update_timestamp.is_some());
        assert_eq!(op_interop(message), complete_metadata());
    }

    async fn assert_final_state(db: &InterchainDatabase) {
        let rows = messages(db).await;
        assert_eq!(rows.len(), 1, "one row per message hash");
        assert_completed(&rows[0]);
        assert_eq!(
            leg_kinds(db).await,
            vec!["executing_message", "relayed_message", "sent_message"]
        );
        assert_eq!(pending_count(db).await, 0, "a completed message is evicted");

        let executions = execution_rows(db).await;
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].message_id, Some(rows[0].id));
        assert_eq!(executions[0].message_hash, Some(MESSAGE_0_ID_HASH.to_vec()));
        assert_eq!(
            (
                executions[0].src_chain_id,
                executions[0].src_block_number,
                executions[0].src_log_index
            ),
            (Some(SOURCE), Some(498_922), Some(1))
        );
        assert_eq!(
            json!(executions[0].log_index),
            op_interop(&rows[0])["executing_message"]["log_index"]
        );
    }

    /// A clone of `log` with the topic at `index` replaced.
    fn with_topic(log: &Log, index: usize, topic: B256) -> Log {
        let mut topics = log.topics().to_vec();
        topics[index] = topic;
        let mut changed = log.clone();
        changed.inner.data = LogData::new_unchecked(topics, log.data().data.clone());
        changed
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_source_then_relay_completes_message() {
        let guard = init_db("op_interop_source_then_relay").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, MessageStatus::Initiated);
        assert_eq!(leg_kinds(&db).await, vec!["sent_message"]);
        assert_eq!(
            op_interop(&rows[0]),
            json!({ "nonce": "0", "source": source_metadata() })
        );

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_relay_then_source_completes_message() {
        let guard = init_db("op_interop_relay_then_source").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();

        assert!(
            messages(&db).await.is_empty(),
            "a destination-only entry writes no canonical row"
        );
        assert_eq!(leg_kinds(&db).await, vec!["executing_message"]);
        assert_eq!(execution_rows(&db).await[0].message_id, None);

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_survives_restart_between_legs_source_first() {
        let guard = init_db("op_interop_restart_source_first").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;

        let first_buffer = new_buffer(&db, Duration::ZERO);
        feed(
            &run_context(&db, first_buffer.clone()),
            SOURCE,
            send_batch(),
        )
        .await;
        first_buffer.run().await.unwrap();
        assert_eq!(messages(&db).await.len(), 1);
        drop(first_buffer);

        let second_buffer = new_buffer(&db, Duration::from_secs(60));
        feed(
            &run_context(&db, second_buffer.clone()),
            DESTINATION,
            relay_logs(),
        )
        .await;
        second_buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_survives_restart_between_legs_destination_first() {
        let guard = init_db("op_interop_restart_destination_first").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;

        let first_buffer = new_buffer(&db, Duration::ZERO);
        feed(
            &run_context(&db, first_buffer.clone()),
            DESTINATION,
            relay_logs(),
        )
        .await;
        first_buffer.run().await.unwrap();
        assert_eq!(
            pending_count(&db).await,
            1,
            "the destination-only entry is offloaded, not lost"
        );
        drop(first_buffer);

        let second_buffer = new_buffer(&db, Duration::from_secs(60));
        feed(
            &run_context(&db, second_buffer.clone()),
            SOURCE,
            send_batch(),
        )
        .await;
        second_buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_both_legs_again_is_idempotent() {
        let guard = init_db("op_interop_idempotent_replay").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_final_state(&db).await;
        let stats_processed = messages(&db).await[0].stats_processed;
        let created_at = execution_rows(&db).await[0].created_at;

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();

        assert_final_state(&db).await;
        assert_eq!(
            messages(&db).await[0].stats_processed,
            stats_processed,
            "a replay must not change the stats marker"
        );
        assert_eq!(execution_rows(&db).await.len(), 1);
        assert_eq!(execution_rows(&db).await[0].created_at, created_at);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_source_only_after_completion_keeps_destination() {
        let guard = init_db("op_interop_source_only_after_completion").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_final_state(&db).await;

        // A retry or rescan re-reads only the source block.
        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_completed(&rows[0]);
        assert_eq!(
            leg_kinds(&db).await,
            vec!["executing_message", "relayed_message", "sent_message"]
        );
        // Known, accepted limitation: the source-only entry cannot finalize
        // again, so it stays in `pending_messages`. It is harmless to the
        // canonical row and is not a leak to chase.
        assert_eq!(pending_count(&db).await, 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_reversed_log_order_gives_same_result() {
        let guard = init_db("op_interop_reversed_log_order").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        // Catch-up batches arrive in descending order.
        let mut descending_relay = relay_logs();
        descending_relay.sort_by_key(|log| std::cmp::Reverse(log.log_index));
        assert!(
            descending_relay[0].log_index > descending_relay[1].log_index,
            "the fixture batch is really reversed"
        );
        let mut descending_send = send_batch();
        descending_send.sort_by_key(|log| std::cmp::Reverse(log.log_index));

        feed(&ctx, DESTINATION, descending_relay).await;
        feed(&ctx, SOURCE, descending_send).await;
        buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_send_to_unconfigured_destination_writes_nothing() {
        let guard = init_db("op_interop_unconfigured_destination").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        let [sent, _] = sent_message_logs();
        let unconfigured = B256::from(U256::from(999_999u64));
        let out_of_range = B256::from(U256::from(1u8) << 200);
        for destination in [unconfigured, out_of_range] {
            // topics: [topic0, destination, target, nonce]
            feed(&ctx, SOURCE, vec![with_topic(&sent, 1, destination)]).await;
        }
        buffer.run().await.unwrap();

        assert!(messages(&db).await.is_empty(), "no canonical row");
        assert!(leg_kinds(&db).await.is_empty(), "no leg row");
        assert_eq!(pending_count(&db).await, 0, "no buffer entry");
        assert!(
            db.open_indexer_failures(&[(BRIDGE_ID, SOURCE)])
                .await
                .unwrap()
                .is_empty(),
            "a skip is not a failure"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_process_batch_log_without_block_timestamp_records_failed_block() {
        let guard = init_db("op_interop_log_without_block_timestamp").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let arc_db = Arc::new(db.clone());
        let ledger = Arc::new(FailureLedger::new(arc_db.clone()));
        let ctx = run_context(&db, new_buffer(&db, Duration::from_secs(60)));

        // An RPC that does not return `blockTimestamp`.
        let mut log = sent_message_logs()[0].clone();
        log.block_timestamp = None;
        let block_number = log.block_number.expect("fixture log has a block");
        let batch = LogBatch {
            from_block: block_number,
            to_block: block_number,
            direction: ScanDirection::Realtime,
            logs: vec![log],
        };
        let settings = FailureRetrySettings {
            enabled: false,
            ..Default::default()
        };

        RangeDriver::new(ctx, ledger, settings)
            .run(vec![(SOURCE, futures::stream::iter(vec![(SOURCE, batch)]))])
            .await
            .expect("a recordable failure must not escalate the driver");

        let open = arc_db
            .open_indexer_failures(&[(BRIDGE_ID, SOURCE)])
            .await
            .unwrap();
        assert_eq!(open.len(), 1, "exactly one open interval: {open:?}");
        assert_eq!(
            open[0].2.range,
            BlockRange {
                from: block_number,
                to: block_number
            }
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_process_batch_invalid_log_dispatches_nothing_from_its_block() {
        let guard = init_db("op_interop_invalid_log_blocks_its_block").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        // The ExecutingMessage row lacks its block hash; the RelayedMessage
        // after it is valid and must not be dispatched on its own.
        let mut logs = relay_logs();
        let executing = logs
            .iter_mut()
            .find(|log| log.log_index == Some(0))
            .expect("fixture has the ExecutingMessage at log index 0");
        executing.block_hash = None;
        let block_number = executing.block_number.expect("fixture log has a block");

        let err = OpInteropIndexer::process_batch(&ctx, DESTINATION, &logs)
            .await
            .expect_err("an invalid log row is a processing error");
        assert_eq!(
            err.attributed,
            vec![BlockRange {
                from: block_number,
                to: block_number
            }]
        );

        buffer.run().await.unwrap();
        assert!(messages(&db).await.is_empty(), "no canonical row");
        assert!(leg_kinds(&db).await.is_empty(), "no leg row");
        assert_eq!(pending_count(&db).await, 0, "no buffer entry");
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_stats_count_message_once_after_completion() {
        let guard = init_db("op_interop_stats_once").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        async fn counted(db: &InterchainDatabase) -> Option<i64> {
            stats_messages::Entity::find()
                .filter(stats_messages::Column::BridgeId.eq(BRIDGE_ID))
                .filter(stats_messages::Column::SrcChainId.eq(SOURCE))
                .filter(stats_messages::Column::DstChainId.eq(DESTINATION))
                .one(db.db.as_ref())
                .await
                .unwrap()
                .map(|row| row.messages_count)
        }

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();
        assert_eq!(messages(&db).await[0].stats_processed, 0);
        assert_eq!(counted(&db).await, None, "initiated is not counted");

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert!(messages(&db).await[0].stats_processed > 0);
        assert_eq!(counted(&db).await, Some(1));

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_eq!(counted(&db).await, Some(1), "a replay does not recount");
    }

    async fn transfers(db: &InterchainDatabase) -> Vec<crosschain_transfers::Model> {
        crosschain_transfers::Entity::find()
            .all(db.db.as_ref())
            .await
            .unwrap()
    }

    fn assert_eth_transfer(transfer: &crosschain_transfers::Model) {
        let amount: sea_orm::prelude::BigDecimal = "1000000000000000000".parse().unwrap();
        assert_eq!(transfer.bridge_id, BRIDGE_ID);
        assert_eq!(transfer.index, 0);
        assert_eq!(transfer.token_src_chain_id, SOURCE);
        assert_eq!(transfer.token_dst_chain_id, DESTINATION);
        assert_eq!(transfer.token_src_address, Some(vec![0; 20]));
        assert_eq!(transfer.token_dst_address, Some(vec![0; 20]));
        assert_eq!(transfer.src_amount, Some(amount.clone()));
        assert_eq!(transfer.dst_amount, Some(amount));
        assert_eq!(transfer.sender_address, Some(vec![0x11; 20]));
        assert_eq!(transfer.recipient_address, Some(vec![0x22; 20]));
        assert_eq!(
            transfer.asset_linkage,
            Some(interchain_indexer_entity::sea_orm_active_enums::TransferAssetLinkage::Mirror)
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_eth_transfer_source_then_relay_writes_one_transfer() {
        let guard = init_db("op_interop_eth_source_then_relay").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        let rows = transfers(&db).await;
        assert_eq!(rows.len(), 1, "the transfer exists from the first flush");
        assert_eth_transfer(&rows[0]);
        assert_eq!(rows[0].stats_processed, 0);

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();

        let rows = transfers(&db).await;
        assert_eq!(rows.len(), 1, "the relay adds no second row");
        assert_eth_transfer(&rows[0]);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_eth_transfer_relay_then_source_writes_one_transfer() {
        let guard = init_db("op_interop_eth_relay_then_source").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert!(transfers(&db).await.is_empty(), "no row until the source");

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        let rows = transfers(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eth_transfer(&rows[0]);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_eth_transfer_survives_restart_between_legs() {
        for source_first in [true, false] {
            let guard = init_db(if source_first {
                "op_interop_eth_restart_source_first"
            } else {
                "op_interop_eth_restart_destination_first"
            })
            .await;
            let db = InterchainDatabase::new(guard.client());
            seed_bridge_and_chains(&db).await;

            let first_buffer = new_buffer(&db, Duration::ZERO);
            let (first_chain, first_logs, second_chain, second_logs) = if source_first {
                (SOURCE, send_batch(), DESTINATION, relay_logs())
            } else {
                (DESTINATION, relay_logs(), SOURCE, send_batch())
            };
            feed(
                &run_context(&db, first_buffer.clone()),
                first_chain,
                first_logs,
            )
            .await;
            first_buffer.run().await.unwrap();
            drop(first_buffer);

            let second_buffer = new_buffer(&db, Duration::from_secs(60));
            feed(
                &run_context(&db, second_buffer.clone()),
                second_chain,
                second_logs,
            )
            .await;
            second_buffer.run().await.unwrap();

            let rows = transfers(&db).await;
            assert_eq!(rows.len(), 1, "source_first = {source_first}");
            assert_eth_transfer(&rows[0]);
        }
    }

    /// The edge of the transfer's asset and its linkage, as database state.
    async fn assert_counted_once(db: &InterchainDatabase) {
        let conn = db.db.as_ref();
        let rows = transfers(db).await;
        assert_eq!(rows.len(), 1);
        let transfer = &rows[0];
        assert_eq!(transfer.stats_processed, 1);
        let asset_id = transfer
            .src_stats_asset_id
            .expect("the source identity resolves");
        assert_eq!(
            transfer.dst_stats_asset_id,
            Some(asset_id),
            "a mirror transfer is one asset"
        );

        let mut linked: Vec<(i64, Vec<u8>)> = stats_asset_tokens::Entity::find()
            .filter(stats_asset_tokens::Column::StatsAssetId.eq(asset_id))
            .all(conn)
            .await
            .unwrap()
            .into_iter()
            .map(|row| (row.chain_id, row.token_address))
            .collect();
        linked.sort();
        assert_eq!(
            linked,
            vec![(SOURCE, vec![0; 20]), (DESTINATION, vec![0; 20])]
        );

        let edges = stats_asset_edges::Entity::find().all(conn).await.unwrap();
        assert_eq!(edges.len(), 1, "one edge: {edges:?}");
        assert_eq!(edges[0].transfers_count, 1);
        assert_eq!(edges[0].decimals, Some(18));
        assert_eq!(
            edges[0].cumulative_amount,
            "1000000000000000000"
                .parse::<sea_orm::prelude::BigDecimal>()
                .unwrap()
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_eth_transfer_stats_counted_once_after_completion() {
        let guard = init_db("op_interop_eth_stats_once").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        seed_native_tokens_into(&db, BRIDGE_ID, &[SOURCE, DESTINATION]).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();
        assert_eq!(transfers(&db).await[0].stats_processed, 0);
        assert!(
            stats_asset_edges::Entity::find()
                .all(db.db.as_ref())
                .await
                .unwrap()
                .is_empty(),
            "an initiated transfer is not counted"
        );

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_counted_once(&db).await;

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_counted_once(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_plain_message_writes_no_transfer() {
        let guard = init_db("op_interop_plain_message_no_transfer").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_calls().remove(1)).await;
        buffer.run().await.unwrap();

        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, MessageStatus::Initiated);
        assert!(transfers(&db).await.is_empty());
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_source_block_adds_transfer_to_message_stored_without_one() {
        let guard = init_db("op_interop_source_block_adds_transfer").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        seed_native_tokens_into(&db, BRIDGE_ID, &[SOURCE, DESTINATION]).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_counted_once(&db).await;

        // What slice 2 left behind: a counted completed message with no
        // transfer and no edge.
        let conn = db.db.as_ref();
        crosschain_transfers::Entity::delete_many()
            .exec(conn)
            .await
            .unwrap();
        stats_asset_edges::Entity::delete_many()
            .exec(conn)
            .await
            .unwrap();
        let message_stats = messages(&db).await[0].stats_processed;

        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        assert_counted_once(&db).await;
        assert_eq!(
            messages(&db).await[0].stats_processed,
            message_stats,
            "the message is not counted a second time"
        );
    }

    async fn native_token(db: &InterchainDatabase, chain_id: i64) -> Option<tokens::Model> {
        db.get_token_info(chain_id as u64, NATIVE_SENTINEL.as_slice().to_vec())
            .await
            .expect("token lookup succeeds")
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_seed_native_tokens_creates_rows_and_is_idempotent() {
        const ICON: &str = "https://example.com/eth.png";
        let guard = init_db("op_interop_seed_native_tokens").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;

        seed_native_tokens_into(&db, BRIDGE_ID, &[SOURCE, DESTINATION]).await;
        for chain_id in [SOURCE, DESTINATION] {
            let row = native_token(&db, chain_id)
                .await
                .expect("one native row per chain");
            assert_eq!(row.r#type, TokenType::Native);
            assert_eq!(row.symbol.as_deref(), Some("ETH"));
            assert_eq!(row.name.as_deref(), Some("Ether"));
            assert_eq!(row.decimals, Some(18));
        }

        let tx = db.db.begin().await.unwrap();
        write_api::set_token_icon_tx(&tx, SOURCE, NATIVE_SENTINEL.as_slice(), Some(ICON))
            .await
            .unwrap()
            .expect("the seeded native row exists");
        tx.commit().await.unwrap();

        seed_native_tokens_into(&db, BRIDGE_ID, &[SOURCE, DESTINATION]).await;
        let row = native_token(&db, SOURCE).await.expect("row survives");
        assert_eq!(row.token_icon.as_deref(), Some(ICON), "the icon survives");
        assert_eq!(row.symbol.as_deref(), Some("ETH"));
        assert_eq!(row.decimals, Some(18));
        assert_eq!(
            tokens::Entity::find().count(db.db.as_ref()).await.unwrap(),
            2,
            "no duplicate rows"
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_foreign_origin_execution_is_stored_unlinked() {
        let guard = init_db("op_interop_foreign_origin_execution").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, DESTINATION, vec![foreign_read(0)]).await;
        buffer.run().await.unwrap();

        let rows = execution_rows(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, DESTINATION);
        assert_eq!(rows[0].log_index, 0);
        assert_eq!(
            (rows[0].message_id, rows[0].message_hash.clone()),
            (None, None)
        );
        assert_eq!(
            (
                rows[0].src_chain_id,
                rows[0].src_block_number,
                rows[0].src_log_index
            ),
            (Some(SOURCE), Some(498_922), Some(1))
        );
        assert_eq!(
            rows[0].event_data["identifier"]["origin"],
            "0x9999999999999999999999999999999999999999"
        );
        assert!(messages(&db).await.is_empty());
        assert_eq!(pending_count(&db).await, 0);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_unpaired_messenger_execution_is_stored_unlinked() {
        let guard = init_db("op_interop_unpaired_messenger_execution").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        feed(
            &ctx,
            DESTINATION,
            vec![crate::indexer::op_interop::test_fixtures::executing_message_log()],
        )
        .await;
        feed(&ctx, SOURCE, send_batch()).await;
        buffer.run().await.unwrap();

        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, MessageStatus::Initiated);
        let executions = execution_rows(&db).await;
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].message_id, None);
        assert_eq!(
            leg_kinds(&db).await,
            vec!["executing_message", "sent_message"]
        );
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_destination_only_after_completion_keeps_the_link() {
        let guard = init_db("op_interop_destination_only_after_completion").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();
        assert_final_state(&db).await;
        let created_at = execution_rows(&db).await[0].created_at;

        feed(&ctx, DESTINATION, relay_logs()).await;
        buffer.run().await.unwrap();

        let executions = execution_rows(&db).await;
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].message_id, Some(messages(&db).await[0].id));
        assert_eq!(executions[0].created_at, created_at);
        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_completed(&rows[0]);
        assert_eq!(pending_count(&db).await, 1);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_pull_read_does_not_steal_the_nesting() {
        let marker = B256::repeat_byte(0xA1);
        for (name, pull_before) in [
            ("op_interop_pull_read_inside_relay", false),
            ("op_interop_pull_read_before_relay", true),
        ] {
            let guard = init_db(name).await;
            let db = InterchainDatabase::new(guard.client());
            seed_bridge_and_chains(&db).await;
            let buffer = new_buffer(&db, Duration::from_secs(60));
            let ctx = run_context(&db, buffer.clone());

            let (logs, own_index, pull_index) = if pull_before {
                (relay_logs_with(pull_read(0, 7, marker), 0), 1, 0)
            } else {
                (relay_logs_with(pull_read(1, 7, marker), 1), 0, 1)
            };
            feed(&ctx, SOURCE, send_batch()).await;
            feed(&ctx, DESTINATION, logs).await;
            buffer.run().await.unwrap();

            let rows = messages(&db).await;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].status, MessageStatus::Completed, "{name}");
            let executions = execution_rows(&db).await;
            assert_eq!(executions.len(), 2, "{name}");
            for exec in &executions {
                let linked = exec.log_index == own_index;
                assert_eq!(exec.message_id.is_some(), linked, "{name}: {exec:?}");
                assert!(linked || exec.log_index == pull_index, "{name}");
            }
            let nested = op_interop(&rows[0]);
            assert_eq!(nested["executing_message"]["log_index"], json!(own_index));
            assert_eq!(nested["destination"]["log_index"], json!(4));
        }
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_slice_three_pending_payload_still_links() {
        let guard = init_db("op_interop_slice_three_pending_payload").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;

        let first_buffer = new_buffer(&db, Duration::ZERO);
        feed(
            &run_context(&db, first_buffer.clone()),
            DESTINATION,
            relay_logs(),
        )
        .await;
        first_buffer.run().await.unwrap();
        assert_eq!(pending_count(&db).await, 1);
        drop(first_buffer);

        let result = db
            .db
            .execute_unprepared(
                "UPDATE pending_messages
                 SET payload = jsonb_set(
                         payload #- '{inner,destination,executing_candidates}',
                         '{inner,destination,executing}',
                         payload #> '{inner,destination,executing_candidates,0}')
                 WHERE bridge_id = 2004;",
            )
            .await
            .unwrap();
        assert_eq!(result.rows_affected(), 1);
        let stored = pending_messages::Entity::find()
            .one(db.db.as_ref())
            .await
            .unwrap()
            .expect("one pending row");
        assert!(
            stored.payload["inner"]["destination"]
                .get("executing_candidates")
                .is_none(),
            "{}",
            stored.payload
        );

        let second_buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, second_buffer.clone());
        feed(&ctx, SOURCE, send_batch()).await;
        second_buffer.run().await.unwrap();

        assert_final_state(&db).await;
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_replay_raw_rows_reach_neither_messages_nor_stats() {
        let guard = init_db("op_interop_raw_rows_reach_nothing_else").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        seed_native_tokens_into(&db, BRIDGE_ID, &[SOURCE, DESTINATION]).await;
        let buffer = new_buffer(&db, Duration::ZERO);
        let ctx = run_context(&db, buffer.clone());

        feed(&ctx, SOURCE, send_batch()).await;
        feed(&ctx, DESTINATION, relay_logs()).await;
        feed(&ctx, DESTINATION, vec![foreign_read(10)]).await;
        feed(
            &ctx,
            DESTINATION,
            vec![pull_read(11, 7, B256::repeat_byte(0xA1))],
        )
        .await;
        buffer.run().await.unwrap();

        assert_eq!(messages(&db).await.len(), 1);
        assert_eq!(transfers(&db).await.len(), 1);
        let stats = stats_messages::Entity::find()
            .filter(stats_messages::Column::BridgeId.eq(BRIDGE_ID))
            .filter(stats_messages::Column::SrcChainId.eq(SOURCE))
            .filter(stats_messages::Column::DstChainId.eq(DESTINATION))
            .one(db.db.as_ref())
            .await
            .unwrap()
            .expect("the completed message is counted");
        assert_eq!(stats.messages_count, 1);
        let edges = stats_asset_edges::Entity::find()
            .all(db.db.as_ref())
            .await
            .unwrap();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].transfers_count, 1);
        let executions = execution_rows(&db).await;
        assert_eq!(executions.len(), 3);
        assert_eq!(
            executions
                .iter()
                .filter(|exec| exec.message_id.is_some())
                .count(),
            1
        );
        assert_eq!(pending_count(&db).await, 0);
    }

    #[tokio::test]
    #[ignore = "needs database"]
    async fn test_process_batch_raw_write_failure_attributes_the_execution_blocks() {
        let guard = init_db("op_interop_raw_write_failure").await;
        let db = InterchainDatabase::new(guard.client());
        seed_bridge_and_chains(&db).await;
        let buffer = new_buffer(&db, Duration::from_secs(60));
        let ctx = run_context(&db, buffer.clone());

        db.db
            .execute_unprepared(
                "CREATE FUNCTION test_refuse_op_interop_legs() RETURNS trigger LANGUAGE plpgsql AS
                   $$ BEGIN RAISE EXCEPTION 'test: op_interop_legs write refused'; END $$;
                 CREATE TRIGGER test_refuse_op_interop_legs BEFORE INSERT ON op_interop_legs
                   FOR EACH ROW EXECUTE FUNCTION test_refuse_op_interop_legs();",
            )
            .await
            .unwrap();

        let err = OpInteropIndexer::process_batch(&ctx, DESTINATION, &relay_logs())
            .await
            .expect_err("the raw write is refused");
        assert_eq!(
            err.attributed,
            vec![BlockRange {
                from: 895_701,
                to: 895_701
            }]
        );
        OpInteropIndexer::process_batch(&ctx, SOURCE, &send_batch())
            .await
            .unwrap_or_else(|err| {
                panic!("a batch without executions writes nothing: {:#}", err.error)
            });

        db.db
            .execute_unprepared(
                "DROP TRIGGER test_refuse_op_interop_legs ON op_interop_legs;
                 DROP FUNCTION test_refuse_op_interop_legs();",
            )
            .await
            .unwrap();
        buffer.run().await.unwrap();

        // The relay's block applied nothing to the buffer: the message waits
        // for the replay instead of completing ahead of it.
        let rows = messages(&db).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].status, MessageStatus::Initiated);
        assert!(execution_rows(&db).await.is_empty());

        // The replay the failure ledger would run.
        OpInteropIndexer::process_batch(&ctx, DESTINATION, &relay_logs())
            .await
            .unwrap_or_else(|err| panic!("the replay succeeds: {:#}", err.error));
        buffer.run().await.unwrap();

        assert_final_state(&db).await;
        assert_eq!(pending_count(&db).await, 0, "no one-sided leftover entry");
    }
}
