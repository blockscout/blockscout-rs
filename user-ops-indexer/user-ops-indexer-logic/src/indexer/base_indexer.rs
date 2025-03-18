use crate::{
    indexer::{
        rpc_utils::{CallTracer, TraceClient, TraceType},
        settings::IndexerSettings,
        status::{EntryPointIndexerStatusMessage, IndexerStatusMessage},
    },
    repository,
    types::user_op::UserOp,
};
use alloy::{
    consensus::Transaction,
    primitives::{Address, BlockHash, Bytes, TxHash, B256},
    providers::Provider,
    rpc::types::{Filter, Log, TransactionReceipt},
    sol_types::{self, SolEvent},
    transports::{TransportError, TransportErrorKind},
};
use anyhow::{anyhow, bail};
use futures::{
    stream::{self, unfold, BoxStream},
    FutureExt, Stream, StreamExt, TryStreamExt,
};
use sea_orm::DatabaseConnection;
use std::{
    future::{self, Future},
    num::NonZeroUsize,
    sync::Arc,
    time::{self, Duration},
};
use tokio::{sync::mpsc, time::sleep};
use tracing::instrument;

#[derive(Hash, Default, Eq, PartialEq)]
struct Job {
    tx_hash: TxHash,
    block_hash: BlockHash,
}

impl From<TxHash> for Job {
    fn from(hash: TxHash) -> Self {
        Self {
            tx_hash: hash,
            block_hash: BlockHash::ZERO,
        }
    }
}

impl TryFrom<Log> for Job {
    type Error = anyhow::Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        if log.removed {
            bail!("unexpected pending log")
        }
        let tx_hash = log
            .transaction_hash
            .ok_or(anyhow::anyhow!("unexpected pending log"))?;
        let block_hash = log
            .block_hash
            .ok_or(anyhow::anyhow!("unexpected pending log"))?;
        Ok(Self {
            tx_hash,
            block_hash,
        })
    }
}

pub trait IndexerLogic {
    const VERSION: &'static str;

    const USER_OPERATION_EVENT_SIGNATURE: B256;

    const BEFORE_EXECUTION_SIGNATURE: B256;

    fn entry_point(&self) -> Address;

    fn matches_handler_calldata(calldata: &Bytes) -> bool;

    fn parse_user_ops(
        &self,
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>>;
    fn user_operation_event_matcher(&self, log: &Log) -> bool {
        log.address() == self.entry_point()
            && log.topic0() == Some(&Self::USER_OPERATION_EVENT_SIGNATURE)
    }

    fn before_execution_matcher(&self, log: &Log) -> bool {
        log.address() == self.entry_point()
            && log.topic0() == Some(&Self::BEFORE_EXECUTION_SIGNATURE)
    }

    fn match_and_parse<T: SolEvent>(&self, log: &Log) -> Option<sol_types::Result<T>> {
        if log.address() == self.entry_point() && log.topic0() == Some(&T::SIGNATURE_HASH) {
            Some(T::decode_log(&log.inner, true).map(|l| l.data))
        } else {
            None
        }
    }
    fn base_tx_logs_filter(&self) -> Filter {
        Filter::new()
            .address(self.entry_point())
            .event_signature(Self::BEFORE_EXECUTION_SIGNATURE)
    }
}

pub struct Indexer<P: Provider, L: IndexerLogic + Sync + Send> {
    client: P,

    db: Arc<DatabaseConnection>,

    settings: IndexerSettings,

    logic: L,

    tx: mpsc::Sender<IndexerStatusMessage>,
}

impl<P: Provider, L: IndexerLogic + Sync + Send> Indexer<P, L> {
    pub fn new(
        client: P,
        db: Arc<DatabaseConnection>,
        settings: IndexerSettings,
        logic: L,
        tx: mpsc::Sender<IndexerStatusMessage>,
    ) -> Self {
        Self {
            client,
            db,
            settings,
            logic,
            tx,
        }
    }

    #[instrument(name = "indexer", skip_all, level = "info", fields(version = L::VERSION))]
    pub async fn start(&self) -> anyhow::Result<()> {
        let trace_client = match self.settings.trace_client {
            Some(client) => client,
            None => {
                tracing::debug!("fetching node client");
                let client_version = self.client.get_client_version().await?;
                tracing::info!(client_version, "fetched node client");
                TraceClient::default_for(client_version.into())
            }
        };

        tracing::debug!("fetching latest block number");
        let block_number = self.client.get_block_number().await?;
        tracing::info!(block_number, "latest block number");

        let mut stream_jobs = stream::SelectAll::<BoxStream<Job>>::new();

        if self.settings.realtime.enabled {
            if self.client.client().pubsub_frontend().is_some() {
                // subscribe to a stream of new logs starting at the current block
                tracing::info!("subscribing to BeforeExecution logs from rpc");
                let realtime_stream_jobs = self
                    .client
                    .subscribe_logs(&self.logic.base_tx_logs_filter())
                    .await?
                    .into_stream()
                    .filter_map(|log| async { Job::try_from(log).ok() });

                // That's the only infinite stream in the SelectAll set. If the ws connection
                // unexpectedly disconnects, this stream will terminate,
                // so will the whole SelectAll set with for_each_concurrent on it.
                // alloy-rs will try to reconnect once, and if failed,
                // the indexer will be restarted with a new rpc provider.
                stream_jobs.push(Box::pin(realtime_stream_jobs));
            } else {
                tracing::info!("starting polling of past BeforeExecution logs from rpc");
                stream_jobs.push(Box::pin(self.poll_for_realtime_jobs(block_number)));
            }
        }

        let rpc_refetch_block_number =
            block_number.saturating_sub(self.settings.past_rpc_logs_indexer.block_range as u64);
        if self.settings.past_db_logs_indexer.enabled {
            let past_db_logs_start_block = self.settings.past_db_logs_indexer.start_block;
            let past_db_logs_end_block = self.settings.past_db_logs_indexer.end_block;
            let from_block = if past_db_logs_start_block > 0 {
                past_db_logs_start_block as u64
            } else {
                rpc_refetch_block_number.saturating_add_signed(past_db_logs_start_block as i64)
            };
            let to_block = if past_db_logs_end_block > 0 {
                past_db_logs_end_block as u64
            } else {
                rpc_refetch_block_number.saturating_add_signed(past_db_logs_end_block as i64)
            };
            tracing::info!(from_block, to_block, "fetching missed tx hashes in db");
            let missed_txs = repository::user_op::stream_unprocessed_logs_tx_hashes(
                &self.db,
                self.logic.entry_point(),
                L::USER_OPERATION_EVENT_SIGNATURE,
                from_block,
                to_block,
            )
            .await?
            .map(Job::from);

            stream_jobs.push(Box::pin(missed_txs.do_after(self.tx.send(
                IndexerStatusMessage::new(
                    L::VERSION,
                    EntryPointIndexerStatusMessage::PastDbLogsIndexingFinished,
                ),
            ))));
        }

        if self.settings.past_rpc_logs_indexer.enabled {
            let jobs = self
                .fetch_jobs_for_block_range(rpc_refetch_block_number + 1, block_number)
                .await?;

            stream_jobs.push(Box::pin(stream::iter(jobs).do_after(self.tx.send(
                IndexerStatusMessage::new(
                    L::VERSION,
                    EntryPointIndexerStatusMessage::PastRpcLogsIndexingFinished,
                ),
            ))));
        }

        let cache_size =
            NonZeroUsize::new(self.settings.deduplication_cache_size).unwrap_or(NonZeroUsize::MIN);
        let cache = lru::LruCache::new(cache_size);
        // map to transactions hashes containing user ops, deduplicate transaction hashes through LRU cache
        // e.g. [A, A, B, B, B, C, C] -> [A, B, C]
        let stream_txs = stream_jobs
            .scan(cache, |cache, job| {
                let now = time::Instant::now();
                let tx_hash = job.tx_hash;
                match cache.put(job, now) {
                    // if LRU cache has seen the same tx hash recently, we skip it
                    Some(ts) if now < ts + self.settings.deduplication_interval => {
                        future::ready(Some(None))
                    }
                    _ => future::ready(Some(Some(tx_hash))),
                }
            })
            .filter_map(|tx_hash| async move { tx_hash });

        self.tx
            .send(IndexerStatusMessage::new(
                L::VERSION,
                EntryPointIndexerStatusMessage::IndexerStarted,
            ))
            .await?;

        stream_txs
            .map(Ok)
            .try_for_each_concurrent(Some(self.settings.concurrency as usize), |tx| async move {
                let mut backoff = vec![5, 20, 120].into_iter().map(Duration::from_secs);
                while let Err(err) = self.handle_tx(tx, trace_client).await {
                    // terminate stream if WS connection is closed, indexer will be restarted
                    if let Some(TransportError::Transport(TransportErrorKind::BackendGone)) = err.downcast_ref::<TransportError>() {
                        tracing::error!(error = ?err, tx_hash = ?tx, "tx handler failed, ws connection closed, exiting");
                        return Err(err);
                    }

                    match backoff.next() {
                        None => {
                            tracing::error!(error = ?err, tx_hash = ?tx, "tx handler failed, skipping");
                            break;
                        }
                        Some(delay) => {
                            tracing::error!(error = ?err, tx_hash = ?tx, ?delay, "tx handler failed, retrying");
                            sleep(delay).await;
                        }
                    };
                }
                Ok(())
            })
            .await
    }

    async fn fetch_jobs_for_block_range(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> Result<Vec<Job>, TransportError> {
        let filter = self
            .logic
            .base_tx_logs_filter()
            .from_block(from_block)
            .to_block(to_block);

        tracing::info!(
            from_block,
            to_block,
            "fetching past BeforeExecution logs from rpc"
        );
        let jobs: Vec<Job> = self
            .client
            .get_logs(&filter)
            .await?
            .into_iter()
            .filter_map(|log| Job::try_from(log).ok())
            .collect();
        tracing::info!(count = jobs.len(), "fetched past BeforeExecution logs");

        Ok(jobs)
    }

    fn poll_for_realtime_jobs(&self, start_block: u64) -> impl Stream<Item = Job> + '_ {
        unfold(
            (start_block, start_block),
            move |(from_block, current_block)| async move {
                async {
                    if current_block <= from_block {
                        sleep(self.settings.realtime.polling_interval).await;
                        tracing::debug!("fetching latest block number");
                        let current_block = self.client.get_block_number().await?;
                        tracing::info!(current_block, "latest block number");
                        return Ok((vec![], (from_block, current_block)));
                    }

                    let from_block = from_block
                        .saturating_sub(self.settings.realtime.polling_block_range as u64);
                    let to_block = (from_block + self.settings.realtime.max_block_range as u64)
                        .min(current_block);

                    let jobs = self
                        .fetch_jobs_for_block_range(from_block, to_block)
                        .await?;

                    Ok((jobs, (to_block + 1, current_block)))
                }
                .await
                .map_or_else(
                    |err: TransportError| {
                        tracing::error!(error = ?err, "failed to poll for logs");
                        Some((vec![], (from_block, current_block)))
                    },
                    Some,
                )
            },
        )
        .flat_map(stream::iter)
    }

    #[instrument(name = "indexer::handle_tx", skip(self, trace_client), level = "info")]
    async fn handle_tx(&self, tx_hash: TxHash, trace_client: TraceClient) -> anyhow::Result<()> {
        let tx = self
            .client
            .get_transaction_by_hash(tx_hash)
            .await?
            .ok_or(anyhow!("empty transaction returned from rpc"))?;

        let receipt = self
            .client
            .get_transaction_receipt(tx_hash)
            .await?
            .ok_or(anyhow!("empty receipt returned from rpc"))?;

        // we split by bundles using BeforeExecution event as a beacon, almost all transaction will contain a single bundle only
        // then we split each bundle into logs batches for respective user operations
        let log_bundles: Vec<Vec<&[Log]>> = receipt
            .inner
            .logs()
            .split(|log| self.logic.before_execution_matcher(log))
            .skip(1)
            .map(|logs| {
                logs.split_inclusive(|log| self.logic.user_operation_event_matcher(log))
                    .filter(|logs| {
                        logs.last()
                            .is_some_and(|log| self.logic.user_operation_event_matcher(log))
                    })
                    .collect()
            })
            .collect();
        tracing::info!(bundles_count = log_bundles.len(), "found user op bundles");

        let calldatas: Vec<Bytes> =
            if log_bundles.len() == 1 && tx.to() == Some(self.logic.entry_point()) {
                vec![tx.input().clone()]
            } else {
                tracing::info!(
                    "tx contains more than one bundle or was sent indirectly, fetching tx trace"
                );
                self.client
                    .common_trace_transaction(tx_hash, trace_client)
                    .await?
                    .into_iter()
                    .filter_map(|t| {
                        if t.typ == TraceType::Call
                            && t.to == Some(self.logic.entry_point())
                            && L::matches_handler_calldata(&t.input)
                        {
                            Some(t.input)
                        } else {
                            None
                        }
                    })
                    .collect()
            };

        if calldatas.len() != log_bundles.len() {
            bail!(
                "number of calls to entrypoint and log batches don't match for {}: {} != {}",
                tx_hash,
                calldatas.len(),
                log_bundles.len()
            )
        }

        let user_ops: Vec<UserOp> = calldatas
            .iter()
            .zip(log_bundles.iter())
            .enumerate()
            .map(|(i, (calldata, log_bundle))| {
                self.logic.parse_user_ops(&receipt, i, calldata, log_bundle)
            })
            .filter_map(|b| {
                // user ops parsing logic won't be retried, since we don't propagate the error here
                b.map_err(|err| tracing::error!(error = ?err, "failed to parse user ops"))
                    .ok()
            })
            .flatten()
            .collect();

        let total = log_bundles.iter().flatten().count();
        let parsed = user_ops.len();
        tracing::info!(
            total,
            parsed,
            missed = total - parsed,
            "found and parsed user ops",
        );
        if parsed > 0 {
            repository::user_op::upsert_many(&self.db, user_ops).await?;
        }

        Ok(())
    }
}

trait StreamEndHook: Stream + Sized
where
    Self::Item: Default,
{
    fn do_after<Fut: Future>(self, f: Fut) -> impl Stream<Item = Self::Item> {
        self.chain(
            f.into_stream()
                .map(|_| Self::Item::default())
                .filter(|_| async { false }),
        )
    }
}

impl<S: Stream> StreamEndHook for S where S::Item: Default {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        indexer::{settings::EntrypointsSettings, v06, v07},
        repository::tests::get_shared_db,
    };
    use alloy::{
        primitives::{address, b256, bytes, U256},
        providers::{ProviderBuilder, RootProvider},
        transports::BoxTransport,
    };
    use entity::sea_orm_active_enums::{EntryPointVersion, SponsorType};
    use sea_orm::EntityTrait;

    const ETH_RPC_URL: &str = "https://eth.drpc.org";

    async fn connect_rpc() -> RootProvider<BoxTransport> {
        ProviderBuilder::new()
            .on_builtin(ETH_RPC_URL)
            .await
            .expect("can't connect")
    }

    #[tokio::test]
    async fn handle_tx_v06_ok() {
        let db = get_shared_db().await;
        // TODO: use mocked connection, alloy::transports::ipc::MockIpcServer from alloy seems broken at v0.8.3
        let client = connect_rpc().await;

        // just some random tx from mainnet
        let tx_hash = b256!("f9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e");
        let entry_point = EntrypointsSettings::default().v06_entry_point;

        let (tx, _) = mpsc::channel(100);
        let indexer = Indexer::new(
            client,
            db.clone(),
            Default::default(),
            v06::IndexerV06 { entry_point },
            tx,
        );
        indexer
            .handle_tx(tx_hash, TraceClient::Trace)
            .await
            .unwrap();

        let op_hash = b256!("2d5f7a884e9a99cfe2445db2af140a8851fbd860852b668f2f199190f68adf87");
        let user_op = repository::user_op::find_user_op_by_op_hash(&db, op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_op, UserOp {
            hash: op_hash,
            sender: address!("eae4d85f7733ad522f601ce7ad4f595704a2d677"),
            nonce: B256::ZERO,
            init_code: None,
            call_data: bytes!("70641a22000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001e0049783f008a0085193e00003d00cd54003c71ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000"),
            call_gas_limit: U256::from(92599),
            verification_gas_limit: U256::from(87149),
            pre_verification_gas: U256::from(49900),
            max_fee_per_gas: U256::from(53825000000u64),
            max_priority_fee_per_gas: U256::from(500000000u64),
            paymaster_and_data: None,
            signature: bytes!("000000000000000000000000000000000000000000000000000000000065793a092c25c7a7c5e4bc46467324e2845caf1ccae767786e07806ca720f8a6b83356bc7d43a63a96b34507cfe7c424db37f351d71851ae9318e8d5c3d9f17c8bdb744c1c"),
            aggregator: None,
            aggregator_signature: None,
            entry_point,
            entry_point_version: EntryPointVersion::V06,
            transaction_hash: tx_hash,
            block_number: 18774992,
            block_hash: b256!("e90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4"),
            bundler: address!("2df993cd76bb8dbda50546eef00eee2e6331a2c8"),
            bundle_index: 0,
            index: 0,
            factory: None,
            paymaster: None,
            status: true,
            revert_reason: None,
            gas: U256::from(229648),
            gas_price: U256::from(37400206579u64),
            gas_used: U256::from(165030),
            sponsor_type: SponsorType::WalletDeposit,
            user_logs_start_index: 268,
            user_logs_count: 1,
            fee: U256::from(6172156091732370u64),
            consensus: None,
            timestamp: None,
        })
    }
    #[tokio::test]
    async fn handle_tx_v06_with_tracing_ok() {
        let db = get_shared_db().await;
        // TODO: use mocked connection, alloy::transports::ipc::MockIpcServer from alloy seems broken at v0.8.3
        let client = connect_rpc().await;

        // just some random tx from mainnet
        let tx_hash = b256!("d69a2233e7ff9034d21f588ffde16ef30dee6ddc4814fa4ecd4a1355630b1730");
        let entry_point = EntrypointsSettings::default().v06_entry_point;
        let op_hash = b256!("e5df829d25b3b0a043a658eb460cf74898eb0ad72a526dba0cd509ed2b83f796");

        let (tx, _) = mpsc::channel(100);
        let indexer = Indexer::new(
            client,
            db.clone(),
            Default::default(),
            v06::IndexerV06 { entry_point },
            tx,
        );
        for trace_client in [TraceClient::Debug, TraceClient::Trace] {
            entity::user_operations::Entity::delete_by_id(op_hash.to_vec())
                .exec(db.as_ref())
                .await
                .expect("failed to delete");

            indexer.handle_tx(tx_hash, trace_client).await.unwrap();

            let user_op = repository::user_op::find_user_op_by_op_hash(&db, op_hash)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(user_op, UserOp {
                hash: op_hash,
                sender: address!("c09521aa72df1f93e36c776d5464f8bf2ae7b37d"),
                nonce: b256!("0000000000000000000000000000000000000000000021050000000000000002"),
                init_code: None,
                call_data: bytes!("2c2abd1e00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000240f0f3f240000000000000000000000005ecd0d3a7f69c73d86f634b52cb9a4c0eb4df7ae00000000000000000000000000000000000000000000000000000000"),
                call_gas_limit: U256::from(89016),
                verification_gas_limit: U256::from(578897),
                pre_verification_gas: U256::from(107592),
                max_fee_per_gas: U256::ZERO,
                max_priority_fee_per_gas: U256::ZERO,
                paymaster_and_data: None,
                signature: bytes!("0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000001700000000000000000000000000000000000000000000000000000000000000013259e664945ba8945c3198dfbfc83dd1c654a7d284dd48b3f4c80544a281938960d7a8386af33457c88801d6e59aa95f374978f0cf400952eea08a15ad68aa7d0000000000000000000000000000000000000000000000000000000000000025f198086b2db17256731bc456673b96bcef23f51d1fbacdd7c4379ef65465572f1d00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008a7b2274797065223a22776562617574686e2e676574222c226368616c6c656e6765223a224d624230784b7177593773684c3461574c31714f5f315668794c4773433632764354682d544342464c704d222c226f726967696e223a2268747470733a2f2f6b6579732e636f696e626173652e636f6d222c2263726f73734f726967696e223a66616c73657d00000000000000000000000000000000000000000000"),
                aggregator: None,
                aggregator_signature: None,
                entry_point,
                entry_point_version: EntryPointVersion::V06,
                transaction_hash: tx_hash,
                block_number: 21478047,
                block_hash: b256!("2389d13b5294f5c38f5120671adf870468d892e60e637cb65e0eb47c455160f9"),
                bundler: address!("c09521aa72df1f93e36c776d5464f8bf2ae7b37d"),
                bundle_index: 0,
                index: 0,
                factory: None,
                paymaster: None,
                status: true,
                revert_reason: None,
                gas: U256::from(775505),
                gas_price: U256::ZERO,
                gas_used: U256::from(428635),
                sponsor_type: SponsorType::WalletBalance,
                user_logs_start_index: 126,
                user_logs_count: 1,
                fee: U256::from(0),
                consensus: None,
                timestamp: None,
            })
        }
    }

    #[tokio::test]
    async fn handle_tx_v07_ok() {
        let db = get_shared_db().await;
        // TODO: use mocked connection, alloy::transports::ipc::MockIpcServer from alloy seems broken at v0.8.3
        let client = connect_rpc().await;

        // just some random tx from mainnet
        let tx_hash = b256!("4a6702f8ef5b7754f5b54dfb00ccba181603e3a6fff77c93e7d0d40148f09ad0");
        let entry_point = EntrypointsSettings::default().v07_entry_point;

        let (tx, _) = mpsc::channel(100);
        let indexer = Indexer::new(
            client,
            db.clone(),
            Default::default(),
            v07::IndexerV07 { entry_point },
            tx,
        );
        indexer
            .handle_tx(tx_hash, TraceClient::Trace)
            .await
            .unwrap();

        let op_hash = b256!("bd48a68e7dd39891fe7f139fe11bfb82d934a5deceb98f0c6fc4ebc7eeca58da");
        let user_op = repository::user_op::find_user_op_by_op_hash(&db, op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_op, UserOp {
            hash: op_hash,
            sender: address!("4a7EFb490b2D34D1962f365C5647D84FAdD3Bd6A"),
            nonce: b256!("00005c97aa67ba578e3c54ec5942a7563ea9130e4f5f4c300000000000000000"),
            init_code: None,
            call_data: bytes!("e9ae5c530100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000006c0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000014000000000000000000000000000000000000000000000000000000000000005a00000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001231deb6f5749ef6ce6943a275a1d3e7486f4eae00000000000000000000000000000000000000000000000000000000000027d9000000000000000000000000000000000000000000000000000000000000000000000000000000001231deb6f5749ef6ce6943a275a1d3e7486f4eae0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000003c44666fc80394356b4034c6f447b20899cff26af1d6d4520e9e3d7616ac0408daeb3cba94a00000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000001000000000000000000000000003c170744609c21c313473e3811563073c850c9f500000000000000000000000000000000000000000000000000000000009032d20000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000000b44656669417070486f6d65000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002a30783030303030303030303030303030303030303030303030303030303030303030303030303030303000000000000000000000000000000000000000000000000000000000000000000000f2614a233c7c3e7f08b1f887ba133a13f1eb2c55000000000000000000000000f2614a233c7c3e7f08b1f887ba133a13f1eb2c550000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800000000000000000000000000000000000000000000000000000000000027d900000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000001442646478b0000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c59900000000000000000000000000000000000000000000000000000000000027d9000000000000000000000000a0b86991c6218b36c1d19d4a2e9eb0ce3606eb4800000000000000000000000000000000000000000000000000000000009032d20000000000000000000000001231deb6f5749ef6ce6943a275a1d3e7486f4eae00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000045022260fac5e5542a773aa44fbcfedf7c193bc2c59901ffff00004375dff511095cc5a197a54140a24efef3a416011231deb6f5749ef6ce6943a275a1d3e7486f4eae000bb800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002260fac5e5542a773aa44fbcfedf7c193bc2c599000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044a9059cbb00000000000000000000000056eb4bf8b7510323b89fde249d1fb660ae30d2ee00000000000000000000000000000000000000000000000000000000000e7e2700000000000000000000000000000000000000000000000000000000"),
            call_gas_limit: U256::from(375068),
            verification_gas_limit: U256::from(166784),
            pre_verification_gas: U256::from(69185),
            max_fee_per_gas: U256::from(1000000),
            max_priority_fee_per_gas: U256::from(5095252459u64),
            paymaster_and_data: Some(bytes!("0000000000000039cd5e8ae05257ce51c473ddd10000000000000000000000000001440600000000000000000000000000000001000000676b732f000000000000bc8f0c8fbbdaa38053b974afd0fe66cc7be40c0941f4ac58304ddda97ee5ba6e14c11f01cbd2c7c46401448bb9615bf29449ead5da1c4755989910524828a2461c")),
            signature: bytes!("f59f5f4d934312578dbd0c7a8a464cd8c73c1cbbb267b1dcae103d0e2d8f3971314b169eac449874652d7f6cb83de63422c762c4dfbca02629f94fc3a478141d1c"),
            aggregator: None,
            aggregator_signature: None,
            entry_point,
            entry_point_version: EntryPointVersion::V07,
            transaction_hash: tx_hash,
            block_number: 21476573,
            block_hash: b256!("54f26503211219a84c1c25d8f5c61d6e1a2f253b97508fb88a6e2e13bffee0b3"),
            bundler: address!("4337003fcD2F56DE3977cCb806383E9161628D0E"),
            bundle_index: 0,
            index: 0,
            factory: None,
            paymaster: Some(address!("0000000000000039cd5e8aE05257CE51C473ddd1")),
            status: true,
            revert_reason: None,
            gas: U256::from(693988),
            gas_price: U256::from(3582831922u64),
            gas_used: U256::from(442171),
            sponsor_type: SponsorType::PaymasterSponsor,
            user_logs_start_index: 458,
            user_logs_count: 11,
            fee: U256::from(1584224373782662u64),
            consensus: None,
            timestamp: None,
        })
    }
}
