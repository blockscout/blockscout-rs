use crate::{
    indexer::v06::constants::{
        matches_entrypoint_event, parse_event, BeforeExecutionFilter, DepositedFilter,
        HandleAggregatedOpsCall, HandleOpsCall, IEntrypointV06Calls, UserOperation,
        UserOperationEventFilter, UserOperationRevertReasonFilter, ENTRYPOINT_V06,
    },
    repository,
    types::user_op::{SponsorType, UserOp},
};
use anyhow::{anyhow, bail};
use ethers::prelude::{BigEndianHash, EthEvent, Middleware, Provider, PubsubClient};
use ethers_core::{
    abi::{AbiDecode, AbiEncode},
    types::{Action, Address, Bytes, Filter, Log},
};
use futures::{stream, stream::BoxStream, StreamExt};
use keccak_hash::H256;
use sea_orm::DatabaseConnection;
use std::{future, ops::Div, time::Duration};
use tokio::time::sleep;

pub struct RawUserOperation {
    pub user_op: UserOperation,

    pub aggregator: Option<Address>,

    pub aggregator_signature: Option<Bytes>,
}

pub struct IndexerV06<'a, C: PubsubClient> {
    client: Provider<C>,

    db: &'a DatabaseConnection,
}

impl<'a, C: PubsubClient> IndexerV06<'a, C> {
    pub fn new(client: Provider<C>, db: &'a DatabaseConnection) -> Self {
        Self { client, db }
    }

    pub async fn start(
        &self,
        concurrency: u32,
        realtime: bool,
        past_rpc_logs_range: u32,
        past_db_logs_start_block: i32,
        past_db_logs_end_block: i32,
    ) -> anyhow::Result<()> {
        let mut stream_txs: BoxStream<H256> = Box::pin(stream::empty());

        let filter = Filter::new()
            .address(*ENTRYPOINT_V06)
            .topic0(BeforeExecutionFilter::signature());

        if realtime {
            // subscribe to a stream of new logs starting at the current block
            tracing::info!("subscribing to BeforeExecution logs from rpc");
            let realtime_stream_txs =
                self.client
                    .subscribe_logs(&filter)
                    .await?
                    .filter_map(|log| {
                        future::ready(if log.removed == Some(true) {
                            log.transaction_hash
                        } else {
                            None
                        })
                    });

            stream_txs = Box::pin(stream_txs.chain(realtime_stream_txs));
        }

        tracing::debug!("fetching latest block number");
        let block_number = self.client.get_block_number().await?.as_u32();
        tracing::info!(block_number, "latest block number");

        let rpc_refetch_block_number = block_number.saturating_sub(past_rpc_logs_range);
        if past_db_logs_start_block != 0 || past_db_logs_end_block != 0 {
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
                self.db,
                *ENTRYPOINT_V06,
                UserOperationEventFilter::signature(),
                from_block,
                to_block,
            )
            .await?;
            tracing::info!(count = txs.len(), "found missed txs in db");

            stream_txs = Box::pin(stream::iter(txs).chain(stream_txs));
        }

        if past_rpc_logs_range > 0 {
            tracing::info!(
                from_block = rpc_refetch_block_number + 1,
                to_block = block_number,
                "fetching past BeforeExecution logs from rpc"
            );
            let filter = filter
                .from_block(rpc_refetch_block_number + 1)
                .to_block(block_number);
            let txs: Vec<H256> = self
                .client
                .get_logs(&filter)
                .await?
                .iter()
                .filter_map(|log| log.transaction_hash)
                .collect();
            tracing::info!(count = txs.len(), "fetched past BeforeExecution logs");

            stream_txs = Box::pin(stream::iter(txs).chain(stream_txs));
        }

        // map to transactions hashes containing user ops, with deduplicated transaction hashes
        // e.g. [A, A, B, B, B, C, C] -> [A, B, C]
        let stream_txs = stream_txs
            .scan(H256::zero(), |prev, tx_hash| {
                if *prev == tx_hash {
                    future::ready(Some(None))
                } else {
                    *prev = tx_hash;
                    future::ready(Some(Some(tx_hash)))
                }
            })
            .filter_map(|tx_hash| async move { tx_hash });

        stream_txs
            .for_each_concurrent(Some(concurrency as usize), |tx| async move {
                let mut backoff = vec![1, 5, 20].into_iter().map(Duration::from_secs);
                while let Err(err) = &self.handle_tx(tx).await {
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

    async fn handle_tx(&self, tx_hash: H256) -> anyhow::Result<()> {
        tracing::info!(tx_hash = ?tx_hash, "processing tx");
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
            .split(matches_entrypoint_event::<BeforeExecutionFilter>)
            .skip(1)
            .map(|logs| {
                logs.split_inclusive(matches_entrypoint_event::<UserOperationEventFilter>)
                    .filter(|logs| {
                        logs.last()
                            .is_some_and(matches_entrypoint_event::<UserOperationEventFilter>)
                    })
                    .collect()
            })
            .collect();
        tracing::info!(tx_hash = ?tx_hash, bundles_count = log_bundles.len(), "found user op bundles for");

        let calldatas: Vec<Bytes> = if log_bundles.len() == 1 && tx.to == Some(*ENTRYPOINT_V06) {
            vec![tx.input]
        } else {
            tracing::info!(tx_hash = ?tx_hash, "tx contains more than one bundle or was sent indirectly, fetching tx trace");
            self.client
                .trace_transaction(tx_hash)
                .await?
                .into_iter()
                .filter_map(|t| {
                    if let Action::Call(cd) = t.action {
                        if cd.to == *ENTRYPOINT_V06 && HandleOpsCall::decode(&cd.input).is_ok()
                            || HandleAggregatedOpsCall::decode(&cd.input).is_ok()
                        {
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

        let tx_deposits: Vec<DepositedFilter> = receipt
            .logs
            .iter()
            .filter_map(|log| parse_event::<DepositedFilter>(log).ok())
            .collect();

        let user_ops: Vec<UserOp> = calldatas
            .iter()
            .zip(log_bundles.iter())
            .enumerate()
            .map(|(i, (calldata, log_bundle))| {
                let calldata = IEntrypointV06Calls::decode(calldata)?;
                let (bundler, raw_user_ops): (Address, Vec<RawUserOperation>) = match calldata {
                    IEntrypointV06Calls::HandleAggregatedOps(cd) => (
                        cd.beneficiary,
                        cd.ops_per_aggregator
                            .into_iter()
                            .flat_map(|agg_ops| {
                                agg_ops
                                    .user_ops
                                    .into_iter()
                                    .map(move |op| RawUserOperation {
                                        user_op: op,
                                        aggregator: Some(agg_ops.aggregator),
                                        aggregator_signature: Some(agg_ops.signature.clone()),
                                    })
                            })
                            .collect(),
                    ),
                    IEntrypointV06Calls::HandleOps(cd) => (
                        cd.beneficiary,
                        cd.ops
                            .into_iter()
                            .map(|op| RawUserOperation {
                                user_op: op,
                                aggregator: None,
                                aggregator_signature: None,
                            })
                            .collect(),
                    ),
                    _ => bail!("can't recognize calldata selector in {}", calldata),
                };
                if raw_user_ops.len() != log_bundle.len() {
                    bail!(
                        "number of user ops in calldata and logs don't match {} != {}",
                        raw_user_ops.len(),
                        log_bundle.len()
                    )
                }
                Ok(raw_user_ops
                    .into_iter()
                    .zip(log_bundle.iter())
                    .enumerate()
                    .filter_map(|(j, (raw_user_op, logs))| {
                        match build_user_op_model(
                            bundler,
                            i as u64,
                            j as u64,
                            raw_user_op,
                            logs,
                            &tx_deposits,
                        ) {
                            Ok(model) => Some(model),
                            Err(err) => {
                                let logs_start_index =
                                    logs.get(0).and_then(|l| l.log_index).map(|i| i.as_u64());
                                let logs_count = logs.len();
                                tracing::error!(
                                    tx_hash = ?tx_hash,
                                    bundle_index = i,
                                    op_index = j,
                                    logs_start_index,
                                    logs_count,
                                    error = ?err,
                                    "failed to parse user op",
                                );
                                None
                            }
                        }
                    })
                    .collect::<Vec<_>>())
            })
            .filter_map(|b| b.ok())
            .flatten()
            .collect();

        let total = log_bundles.iter().flatten().count();
        let parsed = user_ops.len();
        tracing::info!(
            tx_hash = ?tx_hash,
            total,
            parsed,
            missed = total - parsed,
            "found and parsed user ops",
        );
        repository::user_op::upsert_many(self.db, user_ops).await?;

        Ok(())
    }
}

fn build_user_op_model(
    bundler: Address,
    bundle_index: u64,
    index: u64,
    raw_user_op: RawUserOperation,
    logs: &[Log],
    tx_deposits: &[DepositedFilter],
) -> anyhow::Result<UserOp> {
    let user_op_log = logs.last().ok_or(anyhow!("last log missing"))?;
    let user_op_event = parse_event::<UserOperationEventFilter>(user_op_log)?;
    let revert_event = logs
        .iter()
        .find(|&log| matches_entrypoint_event::<UserOperationRevertReasonFilter>(log));
    let revert_event = if let Some(revert_event) = revert_event {
        Some(parse_event::<UserOperationRevertReasonFilter>(
            revert_event,
        )?)
    } else {
        None
    };

    let (call_gas_limit, verification_gas_limit, pre_verification_gas) = (
        raw_user_op.user_op.call_gas_limit.as_u64(),
        raw_user_op.user_op.verification_gas_limit.as_u64(),
        raw_user_op.user_op.pre_verification_gas.as_u64(),
    );

    let factory = if raw_user_op.user_op.init_code.len() >= 20 {
        Some(Address::from_slice(&raw_user_op.user_op.init_code[..20]))
    } else {
        None
    };
    let paymaster = if raw_user_op.user_op.paymaster_and_data.len() >= 20 {
        Some(Address::from_slice(
            &raw_user_op.user_op.paymaster_and_data[..20],
        ))
    } else {
        None
    };
    let sender = raw_user_op.user_op.sender;
    let sender_deposit = tx_deposits.iter().any(|e| e.account == sender);
    let paymaster_deposit = tx_deposits.iter().any(|e| Some(e.account) == paymaster);
    let sponsor_type = match (paymaster, sender_deposit, paymaster_deposit) {
        (None, false, _) => SponsorType::WalletBalance,
        (None, true, _) => SponsorType::WalletDeposit,
        (Some(_), _, false) => SponsorType::PaymasterSponsor,
        (Some(_), _, true) => SponsorType::PaymasterHybrid,
    };
    let mut user_logs_count = logs.len();
    while user_logs_count > 0
        && (logs[user_logs_count - 1].address == *ENTRYPOINT_V06
            || Some(logs[user_logs_count - 1].address) == paymaster)
    {
        user_logs_count -= 1;
    }
    Ok(UserOp {
        hash: H256::from(user_op_event.user_op_hash),
        sender,
        nonce: H256::from_uint(&raw_user_op.user_op.nonce),
        init_code: none_if_empty(raw_user_op.user_op.init_code),
        call_data: raw_user_op.user_op.call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas: raw_user_op.user_op.max_fee_per_gas,
        max_priority_fee_per_gas: raw_user_op.user_op.max_priority_fee_per_gas,
        paymaster_and_data: none_if_empty(raw_user_op.user_op.paymaster_and_data),
        signature: raw_user_op.user_op.signature,
        aggregator: raw_user_op.aggregator,
        aggregator_signature: raw_user_op.aggregator_signature,
        entry_point: *ENTRYPOINT_V06,
        transaction_hash: user_op_log.transaction_hash.unwrap_or(H256::zero()),
        block_number: user_op_log.block_number.map_or(0, |n| n.as_u64()),
        block_hash: user_op_log.block_hash.unwrap_or(H256::zero()),
        bundler,
        bundle_index,
        index,
        factory,
        paymaster,
        status: user_op_event.success,
        revert_reason: revert_event.map(|e| e.revert_reason),
        gas: call_gas_limit
            + verification_gas_limit * if paymaster.is_none() { 1 } else { 3 }
            + pre_verification_gas,
        gas_price: user_op_event
            .actual_gas_cost
            .div(user_op_event.actual_gas_used),
        gas_used: user_op_event.actual_gas_used.as_u64(),
        sponsor_type,
        user_logs_start_index: logs
            .first()
            .map_or(0, |l| l.log_index.map_or(0, |v| v.as_u64())),
        user_logs_count: user_logs_count as u64,
        fee: user_op_event.actual_gas_cost,

        consensus: None,
        timestamp: None,
    })
}

fn none_if_empty(b: Bytes) -> Option<Bytes> {
    if b.is_empty() {
        None
    } else {
        Some(b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::tests::get_shared_db;
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

        let indexer = IndexerV06::new(Provider::new(PubSubMockProvider(client)), &db);
        indexer.handle_tx(tx_hash).await.unwrap();

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
            entry_point: *ENTRYPOINT_V06,
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
