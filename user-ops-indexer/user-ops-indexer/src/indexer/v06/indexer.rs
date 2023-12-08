use std::future;
use std::ops::Div;

use anyhow::{anyhow, bail};
use ethers::prelude::{BigEndianHash, EthEvent, Middleware, Provider, Ws};
use ethers_core::abi::{AbiDecode, AbiEncode};
use ethers_core::types::{Action, Address, Bytes, Filter, Log};
use futures::{stream, StreamExt};
use keccak_hash::H256;
use sea_orm::DatabaseConnection;

use crate::indexer::v06::constants::{
    matches_entrypoint_event, parse_event, BeforeExecutionFilter, DepositedFilter,
    HandleAggregatedOpsCall, HandleOpsCall, IEntrypointV06Calls, UserOperation,
    UserOperationEventFilter, UserOperationRevertReasonFilter, ENTRYPOINT_V06,
};
use crate::repository;
use crate::types::user_op::{SponsorType, UserOp};

pub struct RawUserOperation {
    pub user_op: UserOperation,

    pub aggregator: Option<Address>,

    pub aggregator_signature: Option<Bytes>,
}

pub struct IndexerV06 {
    client: Provider<Ws>,

    db: DatabaseConnection,
}

impl IndexerV06 {
    pub fn new(client: Provider<Ws>, db: DatabaseConnection) -> Self {
        Self { client, db }
    }

    pub async fn start(
        &self,
        past_rpc_logs_range: u32,
        past_db_logs_start_block: i32,
        past_db_logs_end_block: i32,
    ) -> anyhow::Result<()> {
        // subscribe to a stream of new logs starting at the current block
        tracing::info!("subscribing to BeforeExecution logs from rpc");
        let filter = Filter::new()
            .address(*ENTRYPOINT_V06)
            .topic0(BeforeExecutionFilter::signature());
        let stream_logs = self
            .client
            .subscribe_logs(&filter)
            .await?
            .filter(|log| future::ready(log.removed != Some(true)));

        tracing::debug!("fetching latest block number");
        let block_number = self.client.get_block_number().await?.as_u32();
        tracing::info!(block_number, "latest block number");

        let rpc_refetch_block_number = block_number.saturating_sub(past_rpc_logs_range);
        let missed_db_txs = if past_db_logs_start_block != 0 || past_db_logs_end_block != 0 {
            let from_block = if past_db_logs_start_block > 0 {
                past_db_logs_start_block as u64
            } else {
                rpc_refetch_block_number.saturating_sub((-past_db_logs_start_block) as u32) as u64
            };
            let to_block = if past_db_logs_end_block > 0 {
                past_db_logs_end_block as u64
            } else {
                rpc_refetch_block_number.saturating_sub((-past_db_logs_end_block) as u32) as u64
            };
            tracing::info!(from_block, to_block, "fetching missed tx hashes in db");
            let txs = repository::user_op::find_unprocessed_logs_tx_hashes(
                &self.db,
                *ENTRYPOINT_V06,
                UserOperationEventFilter::signature(),
                from_block,
                to_block,
            )
            .await?;
            tracing::info!(count = txs.len(), "found missed txs in db");
            txs
        } else {
            Vec::new()
        };

        let recent_logs = if past_rpc_logs_range > 0 {
            tracing::info!(
                from_block = rpc_refetch_block_number + 1,
                to_block = block_number,
                "fetching past BeforeExecution logs from rpc"
            );
            let filter = filter
                .from_block(rpc_refetch_block_number + 1)
                .to_block(block_number);
            let logs = self.client.get_logs(&filter).await?;
            tracing::info!(count = logs.len(), "fetched past BeforeExecution logs");
            logs
        } else {
            Vec::new()
        };

        let stream_logs = stream::iter(recent_logs).chain(stream_logs);

        // map to transactions hashes containing user ops, with deduplicated transaction hashes
        // e.g. [A, A, B, B, B, C, C] -> [A, B, C]
        let stream_txs = stream::iter(missed_db_txs)
            .chain(stream_logs.filter_map(|log| async move { log.transaction_hash }))
            .scan(H256::zero(), |prev, tx_hash| {
                if *prev == tx_hash {
                    future::ready(Some(None))
                } else {
                    *prev = tx_hash;
                    future::ready(Some(Some(tx_hash.clone())))
                }
            })
            .filter_map(|tx_hash| async move { tx_hash });

        stream_txs
            .for_each(|tx| async move {
                if let Err(err) = &self.handle_tx(tx.clone()).await {
                    tracing::error!(error = ?err, tx_hash = ?tx, "tx handler failed, skipping");
                }
                ()
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

        let tx_deposits = receipt
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
                    .iter()
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
                                let logs_start_index = logs
                                    .get(0)
                                    .and_then(|l| l.log_index)
                                    .map(|i| i.as_u64());
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
        repository::user_op::upsert_many(&self.db, user_ops).await?;

        Ok(())
    }
}

fn build_user_op_model(
    bundler: Address,
    bundle_index: u64,
    op_index: u64,
    raw_user_op: &RawUserOperation,
    logs: &[Log],
    tx_deposits: &Vec<DepositedFilter>,
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
        op_hash: H256::from(user_op_event.user_op_hash),
        sender,
        nonce: H256::from_uint(&raw_user_op.user_op.nonce),
        init_code: none_if_empty(&raw_user_op.user_op.init_code),
        call_data: raw_user_op.user_op.call_data.clone(),
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas: raw_user_op.user_op.max_fee_per_gas,
        max_priority_fee_per_gas: raw_user_op.user_op.max_priority_fee_per_gas,
        paymaster_and_data: none_if_empty(&raw_user_op.user_op.paymaster_and_data),
        signature: raw_user_op.user_op.signature.clone(),
        aggregator: raw_user_op.aggregator,
        aggregator_signature: raw_user_op.aggregator_signature.clone(),
        entry_point: *ENTRYPOINT_V06,
        tx_hash: user_op_log.transaction_hash.unwrap_or(H256::zero()),
        block_number: user_op_log.block_number.map_or(0, |n| n.as_u64()),
        block_hash: user_op_log.block_hash.unwrap_or(H256::zero()),
        bundler,
        bundle_index,
        op_index,
        factory,
        paymaster,
        status: user_op_event.success,
        revert_reason: revert_event.map(|e| e.revert_reason),
        gas: call_gas_limit
            + pre_verification_gas * if paymaster.is_none() { 1 } else { 3 }
            + verification_gas_limit,
        gas_price: user_op_event
            .actual_gas_cost
            .div(user_op_event.actual_gas_used.clone()),
        gas_used: user_op_event.actual_gas_used.as_u64(),
        sponsor_type,
        user_logs_start_index: logs
            .first()
            .map_or(0, |l| l.log_index.map_or(0, |v| v.as_u64())),
        user_logs_count: user_logs_count as u64,

        consensus: None,
        timestamp: None,
    })
}

fn none_if_empty(b: &Bytes) -> Option<Bytes> {
    if b.is_empty() {
        None
    } else {
        Some(b.clone())
    }
}
