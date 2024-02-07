use crate::{
    indexer::{
        base_indexer::IndexerLogic,
        common::{
            extract_address, extract_sponsor_type, extract_user_logs_boundaries, none_if_empty,
            parse_event, unpack_uints,
        },
    },
    types::user_op::UserOp,
};
use anyhow::{anyhow, bail};
use entity::sea_orm_active_enums::EntryPointVersion;
use ethers::prelude::{
    abi::{AbiDecode, Address},
    abigen,
    types::{Bytes, Log, H256},
    BigEndianHash, EthEvent,
};
use lazy_static::lazy_static;
use std::ops::Div;

lazy_static! {
    static ref ENTRYPOINT: ethers::types::Address = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789"
        .parse()
        .unwrap();
}

abigen!(IEntrypointV07, "./src/indexer/v07/abi.json");

pub struct IndexerV07;

struct RawUserOperation {
    pub user_op: PackedUserOperation,

    pub aggregator: Option<Address>,

    pub aggregator_signature: Option<Bytes>,
}

impl IndexerLogic for IndexerV07 {
    fn entry_point() -> Address {
        *ENTRYPOINT
    }

    fn version() -> &'static str {
        "v0.7"
    }

    fn user_operation_event_signature() -> H256 {
        UserOperationEventFilter::signature()
    }

    fn user_operation_revert_reason_signature() -> H256 {
        crate::indexer::v06::UserOperationRevertReasonFilter::signature()
    }

    fn before_execution_signature() -> H256 {
        BeforeExecutionFilter::signature()
    }

    fn deposited_signature() -> H256 {
        crate::indexer::v06::DepositedFilter::signature()
    }

    fn matches_handler_calldata(calldata: &Bytes) -> bool {
        HandleOpsCall::decode(calldata).is_ok() || HandleAggregatedOpsCall::decode(calldata).is_ok()
    }

    fn parse_deposited_event(log: &Log) -> Option<Address> {
        parse_event::<DepositedFilter>(log).ok().map(|e| e.account)
    }

    fn parse_user_ops(
        bundle_index: usize,
        tx_hash: H256,
        tx_deposits: &[Address],
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>> {
        let decoded_calldata = IEntrypointV07Calls::decode(calldata)?;
        let (bundler, raw_user_ops): (Address, Vec<RawUserOperation>) = match decoded_calldata {
            IEntrypointV07Calls::HandleAggregatedOps(cd) => (
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
            IEntrypointV07Calls::HandleOps(cd) => (
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
        .find(|&log| IndexerV07::user_operation_revert_reason_matcher(log))
        .map(parse_event::<UserOperationRevertReasonFilter>)
        .transpose()?;

    let (verification_gas_limit, call_gas_limit) =
        unpack_uints(&raw_user_op.user_op.account_gas_limits[..]);
    let verification_gas_limit = verification_gas_limit.as_u64();
    let call_gas_limit = call_gas_limit.as_u64();
    let pre_verification_gas = raw_user_op.user_op.pre_verification_gas.as_u64();
    let (paymaster_verification_gas_limit, paymaster_post_op_gas_limit) =
        if raw_user_op.user_op.paymaster_and_data.len() >= 52 {
            let (a, b) = unpack_uints(&raw_user_op.user_op.paymaster_and_data[20..52]);
            (a.as_u64(), b.as_u64())
        } else {
            (0, 0)
        };
    let gas = call_gas_limit
        + verification_gas_limit
        + pre_verification_gas
        + paymaster_verification_gas_limit
        + paymaster_post_op_gas_limit;
    let gas_used = user_op_event.actual_gas_used.as_u64();
    let gas_used = gas_used
        + if gas > gas_used {
            (gas - gas_used) / 10
        } else {
            0
        };

    let (max_fee_per_gas, max_priority_fee_per_gas) =
        unpack_uints(&raw_user_op.user_op.gas_fees[..]);

    let factory = extract_address(&raw_user_op.user_op.init_code);
    let paymaster = extract_address(&raw_user_op.user_op.paymaster_and_data);
    let sender = raw_user_op.user_op.sender;
    let (user_logs_start_index, user_logs_count) =
        extract_user_logs_boundaries(logs, *ENTRYPOINT, paymaster);
    Ok(UserOp {
        hash: H256::from(user_op_event.user_op_hash),
        sender,
        nonce: H256::from_uint(&raw_user_op.user_op.nonce),
        init_code: none_if_empty(raw_user_op.user_op.init_code),
        call_data: raw_user_op.user_op.call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data: none_if_empty(raw_user_op.user_op.paymaster_and_data),
        signature: raw_user_op.user_op.signature,
        aggregator: raw_user_op.aggregator,
        aggregator_signature: raw_user_op.aggregator_signature,
        entry_point: *ENTRYPOINT,
        entry_point_version: EntryPointVersion::V07,
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
        gas,
        gas_price: user_op_event.actual_gas_cost.div(gas_used),
        gas_used,
        sponsor_type: extract_sponsor_type(sender, paymaster, tx_deposits),
        user_logs_start_index,
        user_logs_count,
        fee: user_op_event.actual_gas_cost,

        consensus: None,
        timestamp: None,
    })
}
