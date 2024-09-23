use crate::{
    indexer::{
        common_transport::CommonTransport,
        rpc_utils::{to_string, CallTracer, TraceType},
        settings::IndexerSettings,
    },
    repository,
    types::user_op::UserOp,
};
use anyhow::{anyhow, bail};
use ethers::prelude::{
    abi::{AbiEncode, Error},
    parse_log,
    types::{Address, Bytes, Filter, Log, TransactionReceipt},
    EthEvent, Middleware, NodeClient, Provider, ProviderError, H256,
};
use futures::{
    stream,
    stream::{repeat_with, BoxStream},
    Stream, StreamExt,
};
use sea_orm::DatabaseConnection;
use std::{future, num::NonZeroUsize, sync::Arc, time, time::Duration};
use tokio::time::sleep;
use tracing::instrument;

#[derive(Hash, Eq, PartialEq)]
struct Job {
    tx_hash: H256,
    block_hash: H256,
}

impl From<H256> for Job {
    fn from(hash: H256) -> Self {
        Self {
            tx_hash: hash,
            block_hash: H256::zero(),
        }
    }
}

impl TryFrom<Log> for Job {
    type Error = anyhow::Error;

    fn try_from(log: Log) -> Result<Self, Self::Error> {
        if log.removed == Some(true) {
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
    fn entry_point(&self) -> Address;
    fn version() -> &'static str;

    fn user_operation_event_signature() -> H256;

    fn before_execution_signature() -> H256;

    fn matches_handler_calldata(calldata: &Bytes) -> bool;

    fn parse_user_ops(
        &self,
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>>;
    fn user_operation_event_matcher(&self, log: &Log) -> bool {
        log.address == self.entry_point()
            && log.topics.first() == Some(&Self::user_operation_event_signature())
    }

    fn before_execution_matcher(&self, log: &Log) -> bool {
        log.address == self.entry_point()
            && log.topics.first() == Some(&Self::before_execution_signature())
    }

    fn match_and_parse<T: EthEvent>(&self, log: &Log) -> Option<Result<T, Error>> {
        if log.address == self.entry_point() && log.topics.first() == Some(&T::signature()) {
            Some(parse_log::<T>(log.clone()))
        } else {
            None
        }
    }
    fn base_tx_logs_filter(&self) -> Filter {
        Filter::new()
            .address(self.entry_point())
            .topic0(Self::before_execution_signature())
    }
}

pub struct Indexer<L: IndexerLogic + Sync> {
    client: Provider<CommonTransport>,

    db: Arc<DatabaseConnection>,

    settings: IndexerSettings,

    logic: L,
}

impl<L: IndexerLogic + Sync> Indexer<L> {
    pub fn new(
        client: Provider<CommonTransport>,
        db: Arc<DatabaseConnection>,
        settings: IndexerSettings,
        logic: L,
    ) -> Self {
        Self {
            client,
            db,
            settings,
            logic,
        }
    }

    #[instrument(name = "indexer", skip_all, level = "info", fields(version = L::version()))]
    pub async fn start(&self) -> anyhow::Result<()> {
        tracing::debug!("fetching node client");
        let variant = self.client.node_client().await.unwrap_or(NodeClient::Geth);
        tracing::info!(variant = to_string(variant), "fetched node client");

        let mut stream_jobs = stream::SelectAll::<BoxStream<Job>>::new();

        if self.settings.realtime.enabled {
            if self.client.as_ref().supports_subscriptions() {
                // subscribe to a stream of new logs starting at the current block
                tracing::info!("subscribing to BeforeExecution logs from rpc");
                let realtime_stream_jobs = self
                    .client
                    .subscribe_logs(&self.logic.base_tx_logs_filter())
                    .await?
                    .filter_map(|log| async { Job::try_from(log).ok() });

                // That's the only infinite stream in the SelectAll set. If the ws connection
                // unexpectedly disconnects, this stream will terminate,
                // so will the whole SelectAll set with for_each_concurrent on it.
                // ethers-rs does not handle ws reconnects well, neither it can guarantee that no
                // events would be lost even if reconnect is successful, so it's better to restart
                // the whole indexer at once instead of trying to reconnect.
                stream_jobs.push(Box::pin(realtime_stream_jobs));
            } else {
                tracing::info!("starting polling of past BeforeExecution logs from rpc");
                stream_jobs.push(Box::pin(self.poll_for_jobs()));
            }
        }

        tracing::debug!("fetching latest block number");
        let block_number = self.client.get_block_number().await?.as_u32();
        tracing::info!(block_number, "latest block number");

        let rpc_refetch_block_number =
            block_number.saturating_sub(self.settings.past_rpc_logs_indexer.block_range);
        if self.settings.past_db_logs_indexer.enabled {
            let past_db_logs_start_block = self.settings.past_db_logs_indexer.start_block;
            let past_db_logs_end_block = self.settings.past_db_logs_indexer.end_block;
            let from_block = if past_db_logs_start_block > 0 {
                past_db_logs_start_block as u64
            } else {
                rpc_refetch_block_number.saturating_add_signed(past_db_logs_start_block) as u64
            };
            let to_block = if past_db_logs_end_block > 0 {
                past_db_logs_end_block as u64
            } else {
                rpc_refetch_block_number.saturating_add_signed(past_db_logs_end_block) as u64
            };
            tracing::info!(from_block, to_block, "fetching missed tx hashes in db");
            let missed_txs = repository::user_op::stream_unprocessed_logs_tx_hashes(
                &self.db,
                self.logic.entry_point(),
                L::user_operation_event_signature(),
                from_block,
                to_block,
            )
            .await?;

            stream_jobs.push(Box::pin(missed_txs.map(Job::from)));
        }

        if self.settings.past_rpc_logs_indexer.enabled {
            let jobs = self
                .fetch_jobs_for_block_range(rpc_refetch_block_number + 1, block_number)
                .await?;

            stream_jobs.push(Box::pin(stream::iter(jobs)));
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

        stream_txs
            .for_each_concurrent(Some(self.settings.concurrency as usize), |tx| async move {
                let mut backoff = vec![5, 20, 120].into_iter().map(Duration::from_secs);
                while let Err(err) = &self.handle_tx(tx, variant).await {
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
            })
            .await;

        Ok(())
    }

    async fn fetch_jobs_for_block_range(
        &self,
        from_block: u32,
        to_block: u32,
    ) -> Result<Vec<Job>, ProviderError> {
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

    fn poll_for_jobs(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.realtime.polling_interval).await;
            tracing::debug!("fetching latest block number");
            let block_number = self.client.get_block_number().await?.as_u32();
            tracing::info!(block_number, "latest block number");

            let from_block =
                block_number.saturating_sub(self.settings.realtime.polling_block_range);
            let jobs = self
                .fetch_jobs_for_block_range(from_block, block_number)
                .await?;

            Ok::<Vec<Job>, ProviderError>(jobs)
        })
        .filter_map(|fut| async {
            fut.await
                .map_err(|err| tracing::error!(error = ?err, "failed to poll for logs"))
                .ok()
        })
        .flat_map(stream::iter)
    }

    #[instrument(name = "indexer::handle_tx", skip(self, variant), level = "info")]
    async fn handle_tx(&self, tx_hash: H256, variant: NodeClient) -> anyhow::Result<()> {
        let tx = self
            .client
            .get_transaction(tx_hash)
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
            .logs
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
            if log_bundles.len() == 1 && tx.to == Some(self.logic.entry_point()) {
                vec![tx.input]
            } else {
                tracing::info!(
                    "tx contains more than one bundle or was sent indirectly, fetching tx trace"
                );
                self.client
                    .common_trace_transaction(tx_hash, variant)
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
                tx_hash.encode_hex(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        indexer::{v06, v07},
        repository::tests::get_shared_db,
    };
    use entity::sea_orm_active_enums::{EntryPointVersion, SponsorType};
    use ethers::prelude::{MockProvider, Provider};
    use ethers_core::types::{Transaction, TransactionReceipt, U256};
    use std::str::FromStr;

    #[tokio::test]
    async fn handle_tx_v06_ok() {
        let db = get_shared_db().await;
        let client = MockProvider::new();

        // just some random tx from mainnet
        let tx_hash =
            H256::from_str("0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e")
                .unwrap();
        let tx: Transaction = serde_json::from_str(r#"{"accessList":[],"blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","chainId":"0x1","from":"0x2df993cd76bb8dbda50546eef00eee2e6331a2c8","gas":"0x64633","gasPrice":"0x8b539dcf3","hash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","input":"0x1fad948c00000000000000000000000000000000000000000000000000000000000000400000000000000000000000002df993cd76bb8dbda50546eef00eee2e6331a2c800000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d67700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000169b7000000000000000000000000000000000000000000000000000000000001546d000000000000000000000000000000000000000000000000000000000000c2ec0000000000000000000000000000000000000000000000000000000c88385240000000000000000000000000000000000000000000000000000000001dcd650000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000002c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e470641a22000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001e0049783f008a0085193e00003d00cd54003c71ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000062000000000000000000000000000000000000000000000000000000000065793a092c25c7a7c5e4bc46467324e2845caf1ccae767786e07806ca720f8a6b83356bc7d43a63a96b34507cfe7c424db37f351d71851ae9318e8d5c3d9f17c8bdb744c1c000000000000000000000000000000000000000000000000000000000000","maxFeePerGas":"0xc88385240","maxPriorityFeePerGas":"0x1dcd6500","nonce":"0x143","r":"0x1c2b5eb48f71d803de3557309428decfa63f639a97ab98ab6b52667b9c415aa0","s":"0x54a110c5f7db8ce7a488080249d3eab77e426300cd36b78cf82156ded86b26ee","to":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","transactionIndex":"0x63","type":"0x2","v":"0x0","value":"0x0","yParity":"0x0"}"#).unwrap();
        let receipt: TransactionReceipt = serde_json::from_str(r#"{"blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","contractAddress":null,"cumulativeGasUsed":"0xca9e14","effectiveGasPrice":"0x8b539dcf3","from":"0x2df993cd76bb8dbda50546eef00eee2e6331a2c8","gasUsed":"0x27a21","logs":[{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x000000000000000000000000000000000000000000000000002bea15dbb76400","logIndex":"0x10a","removed":false,"topics":["0x2da466a7b24304f47e87fa2e1e5a81b9831ce54fec19055ce277ca2f39ba42c4","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x","logIndex":"0x10b","removed":false,"topics":["0xbb47ee3e183a558b1a2ff0874b079f3fc5478b7454eacf2bfc5af2ff5878f972"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","logIndex":"0x10c","removed":false,"topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677","0x0000000000000000000000001e0049783f008a0085193e00003d00cd54003c71"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000015ed8b1358919200000000000000000000000000000000000000000000000000000000000284a6","logIndex":"0x10d","removed":false,"topics":["0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f","0x2d5f7a884e9a99cfe2445db2af140a8851fbd860852b668f2f199190f68adf87","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677","0x0000000000000000000000000000000000000000000000000000000000000000"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"}],"logsBloom":"0x000000000400000000000000000000000000000000000000000000000000000000080000000000000002000100000000021000000800000000000200002000000000008000000000200000000000000020000000000000000000000000002000000000000a0000000000000000000800000000000000000000000000000200000000000000002000000000000000000000000000000000000000000000000000020001000000400000400000000000000000000020000000000002000000000000000000000000000001000000000000000000000000000000000000000020000050200000000000000000000000000000000000000000000010000000000000","status":"0x1","to":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63","type":"0x2"}"#).unwrap();
        let entry_point = Address::from_str("0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789").unwrap();

        client.push(receipt).unwrap();
        client.push(tx).unwrap();

        let indexer = Indexer::new(
            Provider::new(CommonTransport::Mock(client)),
            db.clone(),
            Default::default(),
            v06::IndexerV06 { entry_point },
        );
        indexer.handle_tx(tx_hash, NodeClient::Geth).await.unwrap();

        let op_hash =
            H256::from_str("0x2d5f7a884e9a99cfe2445db2af140a8851fbd860852b668f2f199190f68adf87")
                .unwrap();
        let user_op = repository::user_op::find_user_op_by_op_hash(&db, op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_op, UserOp {
            hash: op_hash,
            sender: Address::from_str("0xeae4d85f7733ad522f601ce7ad4f595704a2d677").unwrap(),
            nonce: H256::zero(),
            init_code: None,
            call_data: Bytes::from_str("0x70641a22000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001e0049783f008a0085193e00003d00cd54003c71ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00000000000000000000000000000000000000000000000000000000").unwrap(),
            call_gas_limit: U256::from(92599),
            verification_gas_limit: U256::from(87149),
            pre_verification_gas: U256::from(49900),
            max_fee_per_gas: U256::from(53825000000u64),
            max_priority_fee_per_gas: U256::from(500000000u64),
            paymaster_and_data: None,
            signature: Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000065793a092c25c7a7c5e4bc46467324e2845caf1ccae767786e07806ca720f8a6b83356bc7d43a63a96b34507cfe7c424db37f351d71851ae9318e8d5c3d9f17c8bdb744c1c").unwrap(),
            aggregator: None,
            aggregator_signature: None,
            entry_point,
            entry_point_version: EntryPointVersion::V06,
            transaction_hash: tx_hash,
            block_number: 18774992,
            block_hash: H256::from_str("0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4").unwrap(),
            bundler: Address::from_str("0x2df993cd76bb8dbda50546eef00eee2e6331a2c8").unwrap(),
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
    async fn handle_tx_v07_ok() {
        let db = get_shared_db().await;
        let client = MockProvider::new();

        // just some random tx from sepolia
        let tx_hash =
            H256::from_str("0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12")
                .unwrap();
        let tx: Transaction = serde_json::from_str(r#"{"blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","blockNumber":"0x519c6b","from":"0x43d1089285a94bf481e1f6b1a7a114acbc833796","gas":"0x4c4b40","gasPrice":"0xbb3e00f1","maxPriorityFeePerGas":"0xb2d05e00","maxFeePerGas":"0xc20d353e","hash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","input":"0x765e827f000000000000000000000000000000000000000000000000000000000000004000000000000000000000000043d1089285a94bf481e1f6b1a7a114acbc83379600000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000420000000000000000000000000000f4240000000000000000000000000001e8480000000000000000000000000000000000000000000000000000000000007a120000000000000000000000000000000010000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000005a0000000000000000000000000000000000000000000000000000000000000064000000000000000000000000000000000000000000000000000000000000002d81f5806eafab78028b6e29ab65208f54cfdd4ce45a1aafc9e0000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000244ac27308a000000000000000000000000000000000000000000000000000000000000008000000000000000000000000080ee560d57f4b1d2acfeb2174d09d54879c7408800000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000002200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000598991c9d726cbac7eb023ca974fe6e7e7a57ce80000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000003479096622cf141e3cc93126bbccc3ef10b952c1ef000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000074115cff9c5b847b402c382f066cf275ab6440b75aaa1b881c164e5d43131cfb3895759573bc597baf526002f8d1943f1aaa67dbf7fa99cd30d12a235169eef4f3d5c96fc1619c60bc9d8028dfea0f89c7ec5e3f27000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014434fcd5be00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000000000000000000000000000000002540be400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000741b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000061a8000000000000000000000000000061a8000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c800000000000000000000000000000000000000000000000000000002540be400000000000000000000000000000000000000000000000000000000000000000000000000000000000000005a89d0e2cdece3d2f2e2497f2b68c5f96ef073c1800000004200775c0e5049afa24e5370a754faade91452b89dfc97907588ac49b441bcf43d06067f220a252454360907199ae8dfdc7fef2caf6c2aae03e4e0676b2c1ae351601b000000000000","nonce":"0x6","to":"0x0000000071727de22e5e9d8baf0edac6f37da032","transactionIndex":"0x6b","value":"0x0","type":"0x2","accessList":[],"chainId":"0xaa36a7","v":"0x0","r":"0x708c8520e17da32765f6270908ec9961023380a115f6c2a3bbf100f7ef39b68a","s":"0x4730c2959f785391db89cb2cc23db9782054db7d650e7a8df04836e954271d5e"}"#).unwrap();
        let receipt: TransactionReceipt = serde_json::from_str(r#"{"blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","blockNumber":"0x519c6b","contractAddress":null,"cumulativeGasUsed":"0x43e83a","effectiveGasPrice":"0xbb3e00f1","from":"0x43d1089285a94bf481e1f6b1a7a114acbc833796","gasUsed":"0xd0fbb","logs":[{"address":"0xf098c91823f1ef080f22645d030a7196e72d31eb","topics":["0x76329674d4361897f3154af54261c4cc05a0d5964509aeedce71949fa0d34725","0x000000000000000000000000598991c9d726cbac7eb023ca974fe6e7e7a57ce8"],"data":"0x","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x1f","removed":false},{"address":"0xf098c91823f1ef080f22645d030a7196e72d31eb","topics":["0xf80f6dfd1cac76f4ebc9005d547d88739ba90991e2c432ac74b18536c9e72af2"],"data":"0x00000000000000000000000089d0e2cdece3d2f2e2497f2b68c5f96ef073c180","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x20","removed":false},{"address":"0x79096622cf141e3cc93126bbccc3ef10b952c1ef","topics":["0xcdddfb4e53d2f7d725fae607b33383443789359047546dbdbd01f85d21adf61c","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0x","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x21","removed":false},{"address":"0xf098c91823f1ef080f22645d030a7196e72d31eb","topics":["0xb4a437488482177b2d124ce7c50e57d8f8d42a9896b525c9c497ee0d533a95de"],"data":"0x00000000000000000000000079096622cf141e3cc93126bbccc3ef10b952c1ef","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x22","removed":false},{"address":"0x115cff9c5b847b402c382f066cf275ab6440b75a","topics":["0x18c5105ca36f183d9b8ee510786b13e3e58916d2525c72884d40ada1a6112e74","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0xaa1b881c164e5d43131cfb3895759573bc597baf526002f8d1943f1aaa67dbf7fa99cd30d12a235169eef4f3d5c96fc1619c60bc9d8028dfea0f89c7ec5e3f27000000000000000000000000000000000000000000000000000000000002a300","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x23","removed":false},{"address":"0x115cff9c5b847b402c382f066cf275ab6440b75a","topics":["0xcdddfb4e53d2f7d725fae607b33383443789359047546dbdbd01f85d21adf61c","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0x","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x24","removed":false},{"address":"0xf098c91823f1ef080f22645d030a7196e72d31eb","topics":["0xb4a437488482177b2d124ce7c50e57d8f8d42a9896b525c9c497ee0d533a95de"],"data":"0x000000000000000000000000115cff9c5b847b402c382f066cf275ab6440b75a","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x25","removed":false},{"address":"0xf098c91823f1ef080f22645d030a7196e72d31eb","topics":["0xc7f505b2f371ae2175ee4913f4499e1f2633a7b5936321eed1cdaeb6115181d2"],"data":"0x0000000000000000000000000000000000000000000000000000000000000001","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x26","removed":false},{"address":"0x1f5806eafab78028b6e29ab65208f54cfdd4ce45","topics":["0x48df5b960943935df47b5ee244b72a9ea791c73f9d518287bf46d17c8bbe1259","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0x","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x27","removed":false},{"address":"0x0000000071727de22e5e9d8baf0edac6f37da032","topics":["0xd51a9c61267aa6196961883ecf5ff2da6619c37dac0fa92122513fb32c032d2d","0x02bfece5db8c1bd400049c14e20ee988e62c057d296e9aefa34bd9b7f146033e","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0x0000000000000000000000001f5806eafab78028b6e29ab65208f54cfdd4ce450000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x28","removed":false},{"address":"0x0000000071727de22e5e9d8baf0edac6f37da032","topics":["0xbb47ee3e183a558b1a2ff0874b079f3fc5478b7454eacf2bfc5af2ff5878f972"],"data":"0x","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x29","removed":false},{"address":"0x94a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8","topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb","0x0000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e"],"data":"0x00000000000000000000000000000000000000000000000000000002540be400","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x2a","removed":false},{"address":"0x94a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8","topics":["0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb","0x0000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e"],"data":"0x0000000000000000000000000000000000000000000000000000000000000000","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x2b","removed":false},{"address":"0x94a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8","topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb","0x0000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e"],"data":"0x00000000000000000000000000000000000000000000000000000002540be400","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x2c","removed":false},{"address":"0x1b637a3008dc1f86d92031a97fc4b5ac0803329e","topics":["0x17ffde6359ce255c678a17b62fba7f276b9187996206563daaab42c2d836d675","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb"],"data":"0x00000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c80000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000012f4e9","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x2d","removed":false},{"address":"0x0000000071727de22e5e9d8baf0edac6f37da032","topics":["0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f","0x02bfece5db8c1bd400049c14e20ee988e62c057d296e9aefa34bd9b7f146033e","0x000000000000000000000000f098c91823f1ef080f22645d030a7196e72d31eb","0x0000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e"],"data":"0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000001768630000000000000000000000000000000000000000000000000000000000176863","blockNumber":"0x519c6b","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","blockHash":"0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b","logIndex":"0x2e","removed":false}],"logsBloom":"0x02000400001000000200000000010000000180800030000000040000008000000008008801200000004000010000000090000001000000000000020000240000000900080000000000000008000000000040000000000000000008000000900280000000000800000000001000000000000000000000200000000010000002000000000000000000000800404000000000000200000480000000000000000001020000000100400000404000000000000000000004000000000002000010000000002002000000400001008000100400000000040004010000000000000000000010400010100000000000000000000000000000000000000000040000000010","status":"0x1","to":"0x0000000071727de22e5e9d8baf0edac6f37da032","transactionHash":"0xfce54378732b4fdf41a3c65b3b93c6bdabcd0b841bc24969d3593f65ca730f12","transactionIndex":"0x6b","type":"0x2"}"#).unwrap();
        let entry_point = Address::from_str("0x0000000071727De22E5E9d8BAf0edAc6f37da032").unwrap();

        client.push(receipt).unwrap();
        client.push(tx).unwrap();

        let indexer = Indexer::new(
            Provider::new(CommonTransport::Mock(client)),
            db.clone(),
            Default::default(),
            v07::IndexerV07 { entry_point },
        );
        indexer.handle_tx(tx_hash, NodeClient::Geth).await.unwrap();

        let op_hash =
            H256::from_str("0x02bfece5db8c1bd400049c14e20ee988e62c057d296e9aefa34bd9b7f146033e")
                .unwrap();
        let user_op = repository::user_op::find_user_op_by_op_hash(&db, op_hash)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user_op, UserOp {
            hash: op_hash,
            sender: Address::from_str("0xf098c91823f1ef080f22645d030a7196e72d31eb").unwrap(),
            nonce: H256::zero(),
            init_code: Some(Bytes::from_str("0x1f5806eafab78028b6e29ab65208f54cfdd4ce45a1aafc9e0000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000244ac27308a000000000000000000000000000000000000000000000000000000000000008000000000000000000000000080ee560d57f4b1d2acfeb2174d09d54879c7408800000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000002200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000598991c9d726cbac7eb023ca974fe6e7e7a57ce80000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000a0000000000000000000000000000000000000000000000000000000000000003479096622cf141e3cc93126bbccc3ef10b952c1ef000000000000000000000000000000000000000000000000000000000002a3000000000000000000000000000000000000000000000000000000000000000000000000000000000000000074115cff9c5b847b402c382f066cf275ab6440b75aaa1b881c164e5d43131cfb3895759573bc597baf526002f8d1943f1aaa67dbf7fa99cd30d12a235169eef4f3d5c96fc1619c60bc9d8028dfea0f89c7ec5e3f27000000000000000000000000000000000000000000000000000000000002a300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap()),
            call_data: Bytes::from_str("0x34fcd5be00000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c8000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000000000000000000000000000000002540be40000000000000000000000000000000000000000000000000000000000").unwrap(),
            call_gas_limit: U256::from(2000000),
            verification_gas_limit: U256::from(1000000),
            pre_verification_gas: U256::from(500000),
            max_fee_per_gas: U256::from(1),
            max_priority_fee_per_gas: U256::from(1),
            paymaster_and_data: Some(Bytes::from_str("0x1b637a3008dc1f86d92031a97fc4b5ac0803329e00000000000000000000000000061a8000000000000000000000000000061a8000000000000000000000000094a9d9ac8a22534e3faca9f4e7f2e2cf85d5e4c800000000000000000000000000000000000000000000000000000002540be400").unwrap()),
            signature: Bytes::from_str("0x89d0e2cdece3d2f2e2497f2b68c5f96ef073c1800000004200775c0e5049afa24e5370a754faade91452b89dfc97907588ac49b441bcf43d06067f220a252454360907199ae8dfdc7fef2caf6c2aae03e4e0676b2c1ae351601b").unwrap(),
            aggregator: None,
            aggregator_signature: None,
            entry_point,
            entry_point_version: EntryPointVersion::V07,
            transaction_hash: tx_hash,
            block_number: 5348459,
            block_hash: H256::from_str("0x65940368797f7f65885f86fdb367467b2c942aee60ddf9a3fb149a8924ac073b").unwrap(),
            bundler: Address::from_str("0x43d1089285A94bf481E1F6B1a7A114aCBC833796").unwrap(),
            bundle_index: 0,
            index: 0,
            factory: Some(Address::from_str("0x1f5806eAFab78028B6E29Ab65208F54CFdD4ce45").unwrap()),
            paymaster: Some(Address::from_str("0x1b637a3008dc1f86d92031a97FC4B5aC0803329e").unwrap()),
            status: true,
            revert_reason: None,
            gas: U256::from(4300000),
            gas_price: U256::from(1),
            gas_used: U256::from(1534051),
            sponsor_type: SponsorType::PaymasterSponsor,
            user_logs_start_index: 42,
            user_logs_count: 3,
            fee: U256::from(1534051),
            consensus: None,
            timestamp: None,
        })
    }
}
