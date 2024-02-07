use crate::{indexer::base_indexer::IndexerLogic, types::user_op::UserOp};
use anyhow::{anyhow, bail};
use entity::sea_orm_active_enums::{EntryPointVersion, SponsorType};
use ethers::prelude::{
    abi::{AbiDecode, Address, Error, Hash, RawLog},
    abigen,
    types::{Bytes, Log},
    BigEndianHash, EthEvent,
};
use lazy_static::lazy_static;
use std::ops::Div;

lazy_static! {
    static ref ENTRYPOINT: ethers::types::Address = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
        .parse()
        .unwrap();
}

abigen!(IEntrypointV06, "./src/indexer/v06/abi.json");

fn matches_entrypoint_event<T: EthEvent>(log: &Log) -> bool {
    log.address == *ENTRYPOINT && log.topics.first() == Some(&T::signature())
}

fn parse_event<T: EthEvent>(log: &Log) -> Result<T, Error> {
    T::decode_log(&RawLog::from(log.clone()))
}

pub struct IndexerV06;

struct RawUserOperation {
    pub user_op: UserOperation,

    pub aggregator: Option<Address>,

    pub aggregator_signature: Option<Bytes>,
}

impl IndexerLogic for IndexerV06 {
    fn entry_point() -> Address {
        *ENTRYPOINT
    }

    fn version() -> &'static str {
        "v0.6"
    }

    fn user_operation_event_signature() -> Hash {
        UserOperationEventFilter::signature()
    }

    fn before_execution_signature() -> Hash {
        BeforeExecutionFilter::signature()
    }

    fn matches_handler_calldata(calldata: &Bytes) -> bool {
        HandleOpsCall::decode(calldata).is_ok() || HandleAggregatedOpsCall::decode(calldata).is_ok()
    }

    fn parse_deposited_event(log: &Log) -> Option<Address> {
        DepositedFilter::decode_log(&RawLog::from(log.clone()))
            .ok()
            .map(|e| e.account)
    }

    fn parse_user_ops(
        bundle_index: usize,
        tx_hash: Hash,
        tx_deposits: &[Address],
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>> {
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
                    bundle_index as u32,
                    j as u32,
                    raw_user_op,
                    logs,
                    tx_deposits,
                ) {
                    Ok(model) => Some(model),
                    Err(err) => {
                        let logs_start_index =
                            logs.first().and_then(|l| l.log_index).map(|i| i.as_u64());
                        let logs_count = logs.len();
                        tracing::error!(
                            tx_hash = ?tx_hash,
                            bundle_index,
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
    }
}

fn build_user_op_model(
    bundler: Address,
    bundle_index: u32,
    index: u32,
    raw_user_op: RawUserOperation,
    logs: &[Log],
    tx_deposits: &[Address],
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
    let sender_deposit = tx_deposits.iter().any(|&e| e == sender);
    let paymaster_deposit = tx_deposits.iter().any(|&e| Some(e) == paymaster);
    let sponsor_type = match (paymaster, sender_deposit, paymaster_deposit) {
        (None, false, _) => SponsorType::WalletBalance,
        (None, true, _) => SponsorType::WalletDeposit,
        (Some(_), _, false) => SponsorType::PaymasterSponsor,
        (Some(_), _, true) => SponsorType::PaymasterHybrid,
    };
    let mut user_logs_count = logs.len();
    while user_logs_count > 0
        && (logs[user_logs_count - 1].address == *ENTRYPOINT
            || Some(logs[user_logs_count - 1].address) == paymaster)
    {
        user_logs_count -= 1;
    }
    Ok(UserOp {
        hash: Hash::from(user_op_event.user_op_hash),
        sender,
        nonce: Hash::from_uint(&raw_user_op.user_op.nonce),
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
        entry_point: *ENTRYPOINT,
        entry_point_version: EntryPointVersion::V06,
        transaction_hash: user_op_log.transaction_hash.unwrap_or(Hash::zero()),
        block_number: user_op_log.block_number.map_or(0, |n| n.as_u64()),
        block_hash: user_op_log.block_hash.unwrap_or(Hash::zero()),
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
            .map_or(0, |l| l.log_index.map_or(0, |v| v.as_u32())),
        user_logs_count: user_logs_count as u32,
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
