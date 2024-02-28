use crate::{
    indexer::{
        base_indexer::IndexerLogic,
        common::{
            extract_address, extract_sponsor_type, extract_user_logs_boundaries, none_if_empty,
        },
    },
    types::user_op::UserOp,
};
use anyhow::{anyhow, bail};
use entity::sea_orm_active_enums::EntryPointVersion;
use ethers::prelude::{
    abi::{AbiDecode, Address},
    abigen,
    types::{Bytes, Log, TransactionReceipt, H256},
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

pub struct IndexerV06;

struct ExtendedUserOperation {
    user_op: UserOperation,
    bundler: Address,
    aggregator: Option<Address>,
    aggregator_signature: Option<Bytes>,
}

impl IndexerLogic for IndexerV06 {
    fn entry_point() -> Address {
        *ENTRYPOINT
    }

    fn version() -> &'static str {
        "v0.6"
    }

    fn user_operation_event_signature() -> H256 {
        UserOperationEventFilter::signature()
    }

    fn before_execution_signature() -> H256 {
        BeforeExecutionFilter::signature()
    }

    fn matches_handler_calldata(calldata: &Bytes) -> bool {
        HandleOpsCall::decode(calldata).is_ok() || HandleAggregatedOpsCall::decode(calldata).is_ok()
    }

    fn parse_user_ops(
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>> {
        let decoded_calldata = IEntrypointV06Calls::decode(calldata)?;
        let user_ops: Vec<ExtendedUserOperation> = match decoded_calldata {
            IEntrypointV06Calls::HandleAggregatedOps(cd) => cd
                .ops_per_aggregator
                .into_iter()
                .flat_map(|agg_ops| {
                    agg_ops
                        .user_ops
                        .into_iter()
                        .map(move |op| ExtendedUserOperation {
                            user_op: op,
                            bundler: cd.beneficiary,
                            aggregator: Some(agg_ops.aggregator),
                            aggregator_signature: Some(agg_ops.signature.clone()),
                        })
                })
                .collect(),
            IEntrypointV06Calls::HandleOps(cd) => cd
                .ops
                .into_iter()
                .map(|op| ExtendedUserOperation {
                    user_op: op,
                    bundler: cd.beneficiary,
                    aggregator: None,
                    aggregator_signature: None,
                })
                .collect(),
            _ => bail!("can't recognize calldata selector in {}", calldata),
        };
        if user_ops.len() != log_bundle.len() {
            bail!(
                "number of user ops in calldata and logs don't match {} != {}",
                user_ops.len(),
                log_bundle.len()
            )
        }
        Ok(user_ops
            .into_iter()
            .zip(log_bundle.iter())
            .enumerate()
            .filter_map(|(j, (user_op, logs))| {
                match build_user_op_model(receipt, bundle_index as u32, j as u32, user_op, logs) {
                    Ok(model) => Some(model),
                    Err(err) => {
                        let logs_start_index =
                            logs.first().and_then(|l| l.log_index).map(|i| i.as_u64());
                        let logs_count = logs.len();
                        tracing::error!(
                            tx_hash = ?receipt.transaction_hash,
                            bundle_index,
                            op_index = j,
                            logs_start_index,
                            logs_count,
                            error = ?err,
                            "failed to build user op",
                        );
                        None
                    }
                }
            })
            .collect::<Vec<_>>())
    }
}

fn build_user_op_model(
    receipt: &TransactionReceipt,
    bundle_index: u32,
    index: u32,
    user_op: ExtendedUserOperation,
    logs: &[Log],
) -> anyhow::Result<UserOp> {
    let user_op_event = logs
        .last()
        .and_then(IndexerV06::match_and_parse::<UserOperationEventFilter>)
        .transpose()?
        .ok_or(anyhow!("last log doesn't match UserOperationEvent"))?;
    let revert_event = logs
        .iter()
        .find_map(IndexerV06::match_and_parse::<UserOperationRevertReasonFilter>)
        .transpose()?;

    let tx_deposits: Vec<Address> = receipt
        .logs
        .iter()
        .filter_map(IndexerV06::match_and_parse::<DepositedFilter>)
        .filter_map(Result::ok)
        .map(|e| e.account)
        .collect();

    let call_gas_limit = user_op.user_op.call_gas_limit.as_u64();
    let verification_gas_limit = user_op.user_op.verification_gas_limit.as_u64();
    let pre_verification_gas = user_op.user_op.pre_verification_gas.as_u64();

    let factory = extract_address(&user_op.user_op.init_code);
    let paymaster = extract_address(&user_op.user_op.paymaster_and_data);
    let sender = user_op.user_op.sender;
    let (user_logs_start_index, user_logs_count) =
        extract_user_logs_boundaries(logs, *ENTRYPOINT, paymaster);
    Ok(UserOp {
        hash: H256::from(user_op_event.user_op_hash),
        sender,
        nonce: H256::from_uint(&user_op.user_op.nonce),
        init_code: none_if_empty(user_op.user_op.init_code),
        call_data: user_op.user_op.call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas: user_op.user_op.max_fee_per_gas,
        max_priority_fee_per_gas: user_op.user_op.max_priority_fee_per_gas,
        paymaster_and_data: none_if_empty(user_op.user_op.paymaster_and_data),
        signature: user_op.user_op.signature,
        aggregator: user_op.aggregator,
        aggregator_signature: user_op.aggregator_signature,
        entry_point: *ENTRYPOINT,
        entry_point_version: EntryPointVersion::V06,
        transaction_hash: receipt.transaction_hash,
        block_number: receipt.block_number.map_or(0, |n| n.as_u64()),
        block_hash: receipt.block_hash.unwrap_or(H256::zero()),
        bundler: user_op.bundler,
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
        sponsor_type: extract_sponsor_type(sender, paymaster, &tx_deposits),
        user_logs_start_index,
        user_logs_count,
        fee: user_op_event.actual_gas_cost,

        consensus: None,
        timestamp: None,
    })
}
