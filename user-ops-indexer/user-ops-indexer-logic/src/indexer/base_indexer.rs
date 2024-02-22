use crate::{indexer::settings::IndexerSettings, repository, types::user_op::UserOp};
use anyhow::{anyhow, bail};
use ethers::prelude::{
    abi::{AbiEncode, Error},
    parse_log,
    types::{Action, Address, Bytes, Filter, Log, TransactionReceipt},
    EthEvent, Middleware, Provider, ProviderError, PubsubClient, H256,
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
    fn entry_point() -> Address;
    fn version() -> &'static str;

    fn user_operation_event_signature() -> H256;

    fn before_execution_signature() -> H256;

    fn matches_handler_calldata(calldata: &Bytes) -> bool;

    fn parse_user_ops(
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>>;
    fn user_operation_event_matcher(log: &Log) -> bool {
        log.address == Self::entry_point()
            && log.topics.first() == Some(&Self::user_operation_event_signature())
    }

    fn before_execution_matcher(log: &Log) -> bool {
        log.address == Self::entry_point()
            && log.topics.first() == Some(&Self::before_execution_signature())
    }

    fn match_and_parse<T: EthEvent>(log: &Log) -> Option<Result<T, Error>> {
        if log.address == Self::entry_point() && log.topics.first() == Some(&T::signature()) {
            Some(parse_log::<T>(log.clone()))
        } else {
            None
        }
    }
    fn base_tx_logs_filter() -> Filter {
        Filter::new()
            .address(Self::entry_point())
            .topic0(Self::before_execution_signature())
    }
}

pub struct Indexer<C: PubsubClient> {
    client: Provider<C>,

    db: Arc<DatabaseConnection>,

    settings: IndexerSettings,
}

impl<C: PubsubClient> Indexer<C> {
    pub fn new(
        client: Provider<C>,
        db: Arc<DatabaseConnection>,
        settings: IndexerSettings,
    ) -> Self {
        Self {
            client,
            db,
            settings,
        }
    }

    #[instrument(name = "indexer", skip_all, level = "info", fields(version = L::version()))]
    pub async fn start<L: IndexerLogic>(&self, supports_subscriptions: bool) -> anyhow::Result<()> {
        let mut stream_jobs: Vec<BoxStream<Job>> = Vec::new();

        if self.settings.realtime.enabled {
            if supports_subscriptions {
                // subscribe to a stream of new logs starting at the current block
                tracing::info!("subscribing to BeforeExecution logs from rpc");
                let realtime_stream_jobs = self
                    .client
                    .subscribe_logs(&L::base_tx_logs_filter())
                    .await?
                    .filter_map(|log| async { Job::try_from(log).ok() });

                stream_jobs.push(Box::pin(realtime_stream_jobs));
            } else {
                tracing::info!("starting polling of past BeforeExecution logs from rpc");
                stream_jobs.push(Box::pin(self.poll_for_jobs::<L>()));
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
            let txs = repository::user_op::find_unprocessed_logs_tx_hashes(
                &self.db,
                L::entry_point(),
                L::user_operation_event_signature(),
                from_block,
                to_block,
            )
            .await?;
            tracing::info!(count = txs.len(), "found missed txs in db");

            stream_jobs.push(Box::pin(stream::iter(txs).map(Job::from)));
        }

        if self.settings.past_rpc_logs_indexer.enabled {
            let jobs = self
                .fetch_jobs_for_block_range::<L>(rpc_refetch_block_number + 1, block_number)
                .await?;

            stream_jobs.push(Box::pin(stream::iter(jobs)));
        }

        let cache_size =
            NonZeroUsize::new(self.settings.deduplication_cache_size).unwrap_or(NonZeroUsize::MIN);
        let cache = lru::LruCache::new(cache_size);
        // map to transactions hashes containing user ops, deduplicate transaction hashes through LRU cache
        // e.g. [A, A, B, B, B, C, C] -> [A, B, C]
        let stream_txs = stream::select_all(stream_jobs)
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
                let mut backoff = vec![1, 5, 20].into_iter().map(Duration::from_secs);
                while let Err(err) = &self.handle_tx::<L>(tx).await {
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

    async fn fetch_jobs_for_block_range<L: IndexerLogic>(
        &self,
        from_block: u32,
        to_block: u32,
    ) -> Result<Vec<Job>, ProviderError> {
        let filter = L::base_tx_logs_filter()
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

    fn poll_for_jobs<L: IndexerLogic>(&self) -> impl Stream<Item = Job> + '_ {
        repeat_with(|| async {
            sleep(self.settings.realtime.polling_interval).await;
            tracing::debug!("fetching latest block number");
            let block_number = self.client.get_block_number().await?.as_u32();
            tracing::info!(block_number, "latest block number");

            let from_block =
                block_number.saturating_sub(self.settings.realtime.polling_block_range);
            let jobs = self
                .fetch_jobs_for_block_range::<L>(from_block, block_number)
                .await?;

            Ok::<Vec<Job>, ProviderError>(jobs)
        })
        .filter_map(|fut| async { fut.await.ok() })
        .flat_map(stream::iter)
    }

    #[instrument(name = "indexer::handle_tx", skip(self), level = "info")]
    async fn handle_tx<L: IndexerLogic>(&self, tx_hash: H256) -> anyhow::Result<()> {
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
            .split(L::before_execution_matcher)
            .skip(1)
            .map(|logs| {
                logs.split_inclusive(L::user_operation_event_matcher)
                    .filter(|logs| logs.last().is_some_and(L::user_operation_event_matcher))
                    .collect()
            })
            .collect();
        tracing::info!(bundles_count = log_bundles.len(), "found user op bundles");

        let calldatas: Vec<Bytes> = if log_bundles.len() == 1 && tx.to == Some(L::entry_point()) {
            vec![tx.input]
        } else {
            tracing::info!(
                "tx contains more than one bundle or was sent indirectly, fetching tx trace"
            );
            self.client
                .trace_transaction(tx_hash)
                .await?
                .into_iter()
                .filter_map(|t| {
                    if let Action::Call(cd) = t.action {
                        if cd.to == L::entry_point() && L::matches_handler_calldata(&cd.input) {
                            Some(cd.input)
                        } else {
                            None
                        }
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
            .map(|(i, (calldata, log_bundle))| L::parse_user_ops(&receipt, i, calldata, log_bundle))
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
    use crate::{indexer::v06, repository::tests::get_shared_db};
    use entity::sea_orm_active_enums::{EntryPointVersion, SponsorType};
    use ethers::prelude::{JsonRpcClient, MockProvider, Provider};
    use ethers_core::types::{Transaction, TransactionReceipt, U256};
    use futures::stream::{empty, Empty};
    use serde::{de::DeserializeOwned, Serialize};
    use serde_json::value::RawValue;
    use std::{fmt::Debug, future::Future, pin::Pin, str::FromStr};

    #[derive(Debug)]
    struct PubSubMockProvider<M: JsonRpcClient>(M);

    impl<M: JsonRpcClient> JsonRpcClient for PubSubMockProvider<M> {
        type Error = M::Error;

        fn request<'life0, 'life1, 'async_trait, T, R>(
            &'life0 self,
            method: &'life1 str,
            params: T,
        ) -> Pin<Box<dyn Future<Output = Result<R, Self::Error>> + Send + 'async_trait>>
        where
            T: Debug + Serialize + Send + Sync,
            R: DeserializeOwned + Send,
            T: 'async_trait,
            R: 'async_trait,
            'life0: 'async_trait,
            'life1: 'async_trait,
            Self: 'async_trait,
        {
            self.0.request(method, params)
        }
    }

    impl PubsubClient for PubSubMockProvider<MockProvider> {
        type NotificationStream = Empty<Box<RawValue>>;

        fn subscribe<T: Into<U256>>(
            &self,
            _id: T,
        ) -> Result<Self::NotificationStream, Self::Error> {
            Ok(empty())
        }

        fn unsubscribe<T: Into<U256>>(&self, _id: T) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn handle_tx_ok() {
        let db = get_shared_db().await;
        let client = MockProvider::new();

        // just some random tx from mainnet
        let tx_hash =
            H256::from_str("0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e")
                .unwrap();
        let tx: Transaction = serde_json::from_str(r#"{"accessList":[],"blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","chainId":"0x1","from":"0x2df993cd76bb8dbda50546eef00eee2e6331a2c8","gas":"0x64633","gasPrice":"0x8b539dcf3","hash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","input":"0x1fad948c00000000000000000000000000000000000000000000000000000000000000400000000000000000000000002df993cd76bb8dbda50546eef00eee2e6331a2c800000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d67700000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000169b7000000000000000000000000000000000000000000000000000000000001546d000000000000000000000000000000000000000000000000000000000000c2ec0000000000000000000000000000000000000000000000000000000c88385240000000000000000000000000000000000000000000000000000000001dcd650000000000000000000000000000000000000000000000000000000000000002a000000000000000000000000000000000000000000000000000000000000002c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e470641a22000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000001e0049783f008a0085193e00003d00cd54003c71ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000062000000000000000000000000000000000000000000000000000000000065793a092c25c7a7c5e4bc46467324e2845caf1ccae767786e07806ca720f8a6b83356bc7d43a63a96b34507cfe7c424db37f351d71851ae9318e8d5c3d9f17c8bdb744c1c000000000000000000000000000000000000000000000000000000000000","maxFeePerGas":"0xc88385240","maxPriorityFeePerGas":"0x1dcd6500","nonce":"0x143","r":"0x1c2b5eb48f71d803de3557309428decfa63f639a97ab98ab6b52667b9c415aa0","s":"0x54a110c5f7db8ce7a488080249d3eab77e426300cd36b78cf82156ded86b26ee","to":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","transactionIndex":"0x63","type":"0x2","v":"0x0","value":"0x0","yParity":"0x0"}"#).unwrap();
        let receipt: TransactionReceipt = serde_json::from_str(r#"{"blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","contractAddress":null,"cumulativeGasUsed":"0xca9e14","effectiveGasPrice":"0x8b539dcf3","from":"0x2df993cd76bb8dbda50546eef00eee2e6331a2c8","gasUsed":"0x27a21","logs":[{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x000000000000000000000000000000000000000000000000002bea15dbb76400","logIndex":"0x10a","removed":false,"topics":["0x2da466a7b24304f47e87fa2e1e5a81b9831ce54fec19055ce277ca2f39ba42c4","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x","logIndex":"0x10b","removed":false,"topics":["0xbb47ee3e183a558b1a2ff0874b079f3fc5478b7454eacf2bfc5af2ff5878f972"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff","logIndex":"0x10c","removed":false,"topics":["0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677","0x0000000000000000000000001e0049783f008a0085193e00003d00cd54003c71"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"},{"address":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","blockHash":"0xe90aa1d6038c87b029a0666148ac2058ab8397f9c53594cc5a38c0113a48eab4","blockNumber":"0x11e7bd0","data":"0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000015ed8b1358919200000000000000000000000000000000000000000000000000000000000284a6","logIndex":"0x10d","removed":false,"topics":["0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f","0x2d5f7a884e9a99cfe2445db2af140a8851fbd860852b668f2f199190f68adf87","0x000000000000000000000000eae4d85f7733ad522f601ce7ad4f595704a2d677","0x0000000000000000000000000000000000000000000000000000000000000000"],"transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63"}],"logsBloom":"0x000000000400000000000000000000000000000000000000000000000000000000080000000000000002000100000000021000000800000000000200002000000000008000000000200000000000000020000000000000000000000000002000000000000a0000000000000000000800000000000000000000000000000200000000000000002000000000000000000000000000000000000000000000000000020001000000400000400000000000000000000020000000000002000000000000000000000000000001000000000000000000000000000000000000000020000050200000000000000000000000000000000000000000000010000000000000","status":"0x1","to":"0x5ff137d4b0fdcd49dca30c7cf57e578a026d2789","transactionHash":"0xf9f60f6dc99663c6ce4912ef92fe6a122bb90585e47b5f213efca1705be26d6e","transactionIndex":"0x63","type":"0x2"}"#).unwrap();

        client.push(receipt).unwrap();
        client.push(tx).unwrap();

        let indexer = Indexer::new(
            Provider::new(PubSubMockProvider(client)),
            db.clone(),
            Default::default(),
        );
        indexer.handle_tx::<v06::IndexerV06>(tx_hash).await.unwrap();

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
            call_gas_limit: 92599,
            verification_gas_limit: 87149,
            pre_verification_gas: 49900,
            max_fee_per_gas: U256::from(53825000000u64),
            max_priority_fee_per_gas: U256::from(500000000u64),
            paymaster_and_data: None,
            signature: Bytes::from_str("0x000000000000000000000000000000000000000000000000000000000065793a092c25c7a7c5e4bc46467324e2845caf1ccae767786e07806ca720f8a6b83356bc7d43a63a96b34507cfe7c424db37f351d71851ae9318e8d5c3d9f17c8bdb744c1c").unwrap(),
            aggregator: None,
            aggregator_signature: None,
            entry_point: v06::IndexerV06::entry_point(),
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
            gas: 229648,
            gas_price: U256::from(37400206579u64),
            gas_used: 165030,
            sponsor_type: SponsorType::WalletDeposit,
            user_logs_start_index: 268,
            user_logs_count: 1,
            fee: U256::from(6172156091732370u64),
            consensus: None,
            timestamp: None,
        })
    }
}
