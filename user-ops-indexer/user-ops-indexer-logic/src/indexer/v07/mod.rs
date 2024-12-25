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
    primitives::{Address, BlockHash, Bytes, B256, U256},
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
    pub entry_point: Address,
}

struct ExtendedUserOperation {
    user_op: PackedUserOperation,
    bundler: Address,
    aggregator: Option<Address>,
    aggregator_signature: Option<Bytes>,
}

impl IndexerLogic for IndexerV07 {
    fn entry_point(&self) -> Address {
        self.entry_point
    }

    fn version() -> &'static str {
        "v0.7"
    }

    fn user_operation_event_signature() -> B256 {
        IEntrypointV07::UserOperationEvent::SIGNATURE_HASH
    }

    fn before_execution_signature() -> B256 {
        IEntrypointV07::BeforeExecution::SIGNATURE_HASH
    }

    fn matches_handler_calldata(calldata: &Bytes) -> bool {
        IEntrypointV07::handleOpsCall::abi_decode(calldata, true).is_ok()
            || IEntrypointV07::handleAggregatedOpsCall::abi_decode(calldata, true).is_ok()
    }

    fn parse_user_ops(
        &self,
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
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
            .and_then(|log| self.match_and_parse::<IEntrypointV07::UserOperationEvent>(log))
            .transpose()?
            .ok_or(anyhow!("last log doesn't match UserOperationEvent"))?;
        let revert_event = logs
            .iter()
            .find_map(|log| self.match_and_parse::<IEntrypointV07::UserOperationRevertReason>(log))
            .transpose()?;

        let tx_deposits: Vec<Address> = receipt
            .inner
            .logs()
            .iter()
            .filter_map(|log| self.match_and_parse::<IEntrypointV07::Deposited>(log))
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

        let factory = extract_address(&user_op.user_op.initCode);
        let paymaster = extract_address(&user_op.user_op.paymasterAndData);
        let sender = user_op.user_op.sender;
        let (user_logs_start_index, user_logs_count) =
            extract_user_logs_boundaries(logs, self.entry_point, paymaster);
        Ok(UserOp {
            hash: B256::from(user_op_event.userOpHash),
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
            entry_point: self.entry_point,
            entry_point_version: EntryPointVersion::V07,
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
