use crate::{
    indexer::{
        base_indexer::IndexerLogic,
        common::{
            extract_address, extract_sponsor_type, extract_user_logs_boundaries, none_if_empty,
            unpack_uints,
        },
        v07::IEntrypointV07::{IEntrypointV07Calls, PackedUserOperation},
    },
    types::user_op::UserOp,
};
use alloy::{
    primitives::{address, Address, BlockHash, Bytes, B256, U256},
    rpc::types::{Log, TransactionReceipt},
    sol,
    sol_types::{SolCall, SolEvent, SolInterface},
};
use anyhow::{anyhow, bail};
use entity::sea_orm_active_enums::EntryPointVersion;
use std::ops::Div;

sol!(IEntrypointV07, "./src/indexer/v07/abi.json");

#[derive(Debug, Clone)]
pub struct IndexerV07 {
    pub v07_entry_points: Vec<Address>,
    pub v08_entry_points: Vec<Address>,
}

struct ExtendedUserOperation {
    entry_point: Address,
    user_op: PackedUserOperation,
    bundler: Address,
    aggregator: Option<Address>,
    aggregator_signature: Option<Bytes>,
}

impl IndexerLogic for IndexerV07 {
    const VERSION: &'static str = "v0.7-v0.8";

    const USER_OPERATION_EVENT_SIGNATURE: B256 = IEntrypointV07::UserOperationEvent::SIGNATURE_HASH;

    const BEFORE_EXECUTION_SIGNATURE: B256 = IEntrypointV07::BeforeExecution::SIGNATURE_HASH;

    fn entry_points(&self) -> Vec<Address> {
        let mut entry_points =
            Vec::with_capacity(self.v07_entry_points.len() + self.v08_entry_points.len());
        entry_points.extend_from_slice(&self.v07_entry_points);
        entry_points.extend_from_slice(&self.v08_entry_points);
        entry_points
    }

    fn matches_entry_point(&self, address: Address) -> bool {
        self.v07_entry_points.contains(&address) || self.v08_entry_points.contains(&address)
    }

    fn matches_handler_calldata(calldata: &Bytes) -> bool {
        IEntrypointV07::handleOpsCall::abi_decode(calldata, true).is_ok()
            || IEntrypointV07::handleAggregatedOpsCall::abi_decode(calldata, true).is_ok()
    }

    fn parse_user_ops(
        &self,
        receipt: &TransactionReceipt,
        bundle_index: usize,
        entry_point: Address,
        calldata: &Bytes,
        bundle_logs: &[Log],
    ) -> anyhow::Result<Vec<UserOp>> {
        let decoded_calldata = IEntrypointV07Calls::abi_decode(calldata, true)?;
        let user_ops: Vec<ExtendedUserOperation> = match decoded_calldata {
            IEntrypointV07Calls::handleAggregatedOps(cd) => cd
                .opsPerAggregator
                .into_iter()
                .flat_map(|agg_ops| {
                    agg_ops
                        .userOps
                        .into_iter()
                        .map(move |op| ExtendedUserOperation {
                            entry_point,
                            user_op: op,
                            bundler: cd.beneficiary,
                            aggregator: Some(agg_ops.aggregator),
                            aggregator_signature: Some(agg_ops.signature.clone()),
                        })
                })
                .collect(),
            IEntrypointV07Calls::handleOps(cd) => cd
                .ops
                .into_iter()
                .map(|op| ExtendedUserOperation {
                    entry_point,
                    user_op: op,
                    bundler: cd.beneficiary,
                    aggregator: None,
                    aggregator_signature: None,
                })
                .collect(),
            _ => bail!("can't recognize calldata selector in {}", calldata),
        };
        let mut user_ops_logs = Vec::new();
        let mut start = 0;
        let mut end = 0;
        for op in &user_ops {
            while let Some(log) = bundle_logs.get(end) {
                end += 1;
                if let Some(Ok(e)) =
                    self.match_and_parse::<IEntrypointV07::UserOperationEvent>(entry_point, log)
                {
                    if e.sender == op.user_op.sender && e.nonce == op.user_op.nonce {
                        user_ops_logs.push(&bundle_logs[start..end]);
                        start = end;
                        break;
                    }
                }
            }
        }
        if user_ops.len() != user_ops_logs.len() {
            bail!(
                "number of user ops in calldata and logs don't match {} != {}",
                user_ops.len(),
                user_ops_logs.len()
            )
        }
        Ok(user_ops
            .into_iter()
            .zip(user_ops_logs)
            .enumerate()
            .filter_map(|(j, (user_op, logs))| {
                match self.build_user_op_model(
                    receipt,
                    bundle_index as u32,
                    j as u32,
                    user_op,
                    logs,
                ) {
                    Ok(model) => Some(model),
                    Err(err) => {
                        let logs_start_index = logs.first().and_then(|l| l.log_index);
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

impl IndexerV07 {
    fn build_user_op_model(
        &self,
        receipt: &TransactionReceipt,
        bundle_index: u32,
        index: u32,
        user_op: ExtendedUserOperation,
        logs: &[Log],
    ) -> anyhow::Result<UserOp> {
        let user_op_event = logs
            .last()
            .and_then(|log| {
                self.match_and_parse::<IEntrypointV07::UserOperationEvent>(user_op.entry_point, log)
            })
            .transpose()?
            .ok_or(anyhow!("last log doesn't match UserOperationEvent"))?;
        let revert_event = logs
            .iter()
            .find_map(|log| {
                self.match_and_parse::<IEntrypointV07::UserOperationRevertReason>(
                    user_op.entry_point,
                    log,
                )
            })
            .transpose()?;

        let tx_deposits: Vec<Address> = receipt
            .inner
            .logs()
            .iter()
            .filter_map(|log| {
                self.match_and_parse::<IEntrypointV07::Deposited>(user_op.entry_point, log)
            })
            .filter_map(Result::ok)
            .map(|e| e.account)
            .collect();

        let (verification_gas_limit, call_gas_limit) =
            unpack_uints(&user_op.user_op.accountGasLimits[..]);
        let pre_verification_gas = user_op.user_op.preVerificationGas;
        let (paymaster_verification_gas_limit, paymaster_post_op_gas_limit) =
            if user_op.user_op.paymasterAndData.len() >= 52 {
                unpack_uints(&user_op.user_op.paymasterAndData[20..52])
            } else {
                (U256::ZERO, U256::ZERO)
            };
        let gas = call_gas_limit
            + verification_gas_limit
            + pre_verification_gas
            + paymaster_verification_gas_limit
            + paymaster_post_op_gas_limit;

        let (max_fee_per_gas, max_priority_fee_per_gas) =
            unpack_uints(&user_op.user_op.gasFees[..]);

        let factory = match extract_address(&user_op.user_op.initCode) {
            Some(f) if f == address!("7702000000000000000000000000000000000000") => None,
            res => res,
        };
        let paymaster = extract_address(&user_op.user_op.paymasterAndData);
        let sender = user_op.user_op.sender;
        let (user_logs_start_index, user_logs_count) =
            extract_user_logs_boundaries(logs, user_op.entry_point, paymaster);
        Ok(UserOp {
            hash: user_op_event.userOpHash,
            sender,
            nonce: B256::from(user_op.user_op.nonce),
            init_code: none_if_empty(user_op.user_op.initCode),
            call_data: user_op.user_op.callData,
            call_gas_limit,
            verification_gas_limit,
            pre_verification_gas,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            paymaster_and_data: none_if_empty(user_op.user_op.paymasterAndData),
            signature: user_op.user_op.signature,
            aggregator: user_op.aggregator,
            aggregator_signature: user_op.aggregator_signature,
            entry_point: user_op.entry_point,
            entry_point_version: if self.v08_entry_points.contains(&user_op.entry_point) {
                EntryPointVersion::V08
            } else {
                EntryPointVersion::V07
            },
            transaction_hash: receipt.transaction_hash,
            block_number: receipt.block_number.unwrap_or(0),
            block_hash: receipt.block_hash.unwrap_or(BlockHash::ZERO),
            bundler: user_op.bundler,
            bundle_index,
            index,
            factory,
            paymaster,
            status: user_op_event.success,
            revert_reason: revert_event.map(|e| e.revertReason),
            gas,
            gas_price: user_op_event.actualGasCost.div(user_op_event.actualGasUsed),
            gas_used: user_op_event.actualGasUsed,
            sponsor_type: extract_sponsor_type(sender, paymaster, &tx_deposits),
            user_logs_start_index,
            user_logs_count,
            fee: user_op_event.actualGasCost,

            consensus: None,
            timestamp: None,
        })
    }
}
