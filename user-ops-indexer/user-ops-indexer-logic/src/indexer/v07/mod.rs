use crate::{
    indexer::{
        base_indexer::IndexerLogic,
        common::{
            extract_address, extract_sponsor_type, extract_user_logs_boundaries, none_if_empty,
            unpack_uints,
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
    BigEndianHash, EthEvent, U256,
};
use std::ops::Div;

abigen!(IEntrypointV07, "./src/indexer/v07/abi.json");

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
        &self,
        receipt: &TransactionReceipt,
        bundle_index: usize,
        calldata: &Bytes,
        log_bundle: &[&[Log]],
    ) -> anyhow::Result<Vec<UserOp>> {
        let decoded_calldata = IEntrypointV07Calls::decode(calldata)?;
        let user_ops: Vec<ExtendedUserOperation> = match decoded_calldata {
            IEntrypointV07Calls::HandleAggregatedOps(cd) => cd
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
            IEntrypointV07Calls::HandleOps(cd) => cd
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
            .and_then(|log| self.match_and_parse::<UserOperationEventFilter>(log))
            .transpose()?
            .ok_or(anyhow!("last log doesn't match UserOperationEvent"))?;
        let revert_event = logs
            .iter()
            .find_map(|log| self.match_and_parse::<UserOperationRevertReasonFilter>(log))
            .transpose()?;

        let tx_deposits: Vec<Address> = receipt
            .logs
            .iter()
            .filter_map(|log| self.match_and_parse::<DepositedFilter>(log))
            .filter_map(Result::ok)
            .map(|e| e.account)
            .collect();

        let (verification_gas_limit, call_gas_limit) =
            unpack_uints(&user_op.user_op.account_gas_limits[..]);
        let pre_verification_gas = user_op.user_op.pre_verification_gas;
        let (paymaster_verification_gas_limit, paymaster_post_op_gas_limit) =
            if user_op.user_op.paymaster_and_data.len() >= 52 {
                unpack_uints(&user_op.user_op.paymaster_and_data[20..52])
            } else {
                (U256::zero(), U256::zero())
            };
        let gas = call_gas_limit
            + verification_gas_limit
            + pre_verification_gas
            + paymaster_verification_gas_limit
            + paymaster_post_op_gas_limit;

        let (max_fee_per_gas, max_priority_fee_per_gas) =
            unpack_uints(&user_op.user_op.gas_fees[..]);

        let factory = extract_address(&user_op.user_op.init_code);
        let paymaster = extract_address(&user_op.user_op.paymaster_and_data);
        let sender = user_op.user_op.sender;
        let (user_logs_start_index, user_logs_count) =
            extract_user_logs_boundaries(logs, self.entry_point, paymaster);
        Ok(UserOp {
            hash: H256::from(user_op_event.user_op_hash),
            sender,
            nonce: H256::from_uint(&user_op.user_op.nonce),
            init_code: none_if_empty(user_op.user_op.init_code),
            call_data: user_op.user_op.call_data,
            call_gas_limit,
            verification_gas_limit,
            pre_verification_gas,
            max_fee_per_gas,
            max_priority_fee_per_gas,
            paymaster_and_data: none_if_empty(user_op.user_op.paymaster_and_data),
            signature: user_op.user_op.signature,
            aggregator: user_op.aggregator,
            aggregator_signature: user_op.aggregator_signature,
            entry_point: self.entry_point,
            entry_point_version: EntryPointVersion::V07,
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
            gas,
            gas_price: user_op_event
                .actual_gas_cost
                .div(user_op_event.actual_gas_used),
            gas_used: user_op_event.actual_gas_used,
            sponsor_type: extract_sponsor_type(sender, paymaster, &tx_deposits),
            user_logs_start_index,
            user_logs_count,
            fee: user_op_event.actual_gas_cost,

            consensus: None,
            timestamp: None,
        })
    }
}
