use crate::{
    indexer::common::decode_execute_call_data, repository::user_op::ListUserOpDB,
    types::common::u256_to_decimal,
};
use alloy::primitives::ruint::UintTryTo;
use alloy::primitives::{Address, BlockHash, BlockNumber, Bytes, TxHash, B128, B256, U128, U256};
pub use entity::sea_orm_active_enums::{EntryPointVersion, SponsorType};
use entity::user_operations::Model;
use num_traits::cast::ToPrimitive;
use sea_orm::ActiveEnum;
use std::ops::Mul;

#[derive(Clone, Debug, PartialEq)]
pub struct UserOp {
    pub hash: B256,
    pub sender: Address,
    pub nonce: B256,
    pub init_code: Option<Bytes>,
    pub call_data: Bytes,
    pub call_gas_limit: U256,
    pub verification_gas_limit: U256,
    pub pre_verification_gas: U256,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub paymaster_and_data: Option<Bytes>,
    pub signature: Bytes,
    pub aggregator: Option<Address>,
    pub aggregator_signature: Option<Bytes>,
    pub entry_point: Address,
    pub entry_point_version: EntryPointVersion,
    pub transaction_hash: TxHash,
    pub block_number: BlockNumber,
    pub block_hash: BlockHash,
    pub bundler: Address,
    pub bundle_index: u32,
    pub index: u32,
    pub factory: Option<Address>,
    pub paymaster: Option<Address>,
    pub status: bool,
    pub revert_reason: Option<Bytes>,
    pub gas: U256,
    pub gas_price: U256,
    pub gas_used: U256,
    pub sponsor_type: SponsorType,
    pub user_logs_start_index: u32,
    pub user_logs_count: u32,
    pub fee: U256,

    pub consensus: Option<bool>,
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListUserOp {
    pub hash: B256,
    pub entry_point: Address,
    pub entry_point_version: EntryPointVersion,
    pub block_number: u64,
    pub sender: Address,
    pub transaction_hash: TxHash,
    pub timestamp: String,
    pub status: bool,
    pub fee: U256,
}

impl From<UserOp> for Model {
    fn from(v: UserOp) -> Self {
        Self {
            hash: v.hash.to_vec(),
            sender: v.sender.to_vec(),
            nonce: v.nonce.to_vec(),
            init_code: v.init_code.clone().map(|a| a.to_vec()),
            call_data: v.call_data.to_vec(),
            call_gas_limit: u256_to_decimal(v.call_gas_limit),
            verification_gas_limit: u256_to_decimal(v.verification_gas_limit),
            pre_verification_gas: u256_to_decimal(v.pre_verification_gas),
            max_fee_per_gas: u256_to_decimal(v.max_fee_per_gas),
            max_priority_fee_per_gas: u256_to_decimal(v.max_priority_fee_per_gas),
            paymaster_and_data: v.paymaster_and_data.clone().map(|a| a.to_vec()),
            signature: v.signature.to_vec(),
            aggregator: v.aggregator.map(|a| a.to_vec()),
            aggregator_signature: v.aggregator_signature.clone().map(|a| a.to_vec()),
            entry_point: v.entry_point.to_vec(),
            entry_point_version: v.entry_point_version.clone(),
            transaction_hash: v.transaction_hash.to_vec(),
            block_number: v.block_number as i32,
            block_hash: v.block_hash.to_vec(),
            bundler: v.bundler.to_vec(),
            bundle_index: v.bundle_index as i32,
            index: v.index as i32,
            factory: v.factory.map(|a| a.to_vec()),
            paymaster: v.paymaster.map(|a| a.to_vec()),
            status: v.status,
            revert_reason: v.revert_reason.clone().map(|a| a.to_vec()),
            gas: u256_to_decimal(v.gas),
            gas_price: u256_to_decimal(v.gas_price),
            gas_used: u256_to_decimal(v.gas_used),
            sponsor_type: v.sponsor_type.clone(),
            user_logs_start_index: v.user_logs_start_index as i32,
            user_logs_count: v.user_logs_count as i32,
            inserted_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

impl From<Model> for UserOp {
    fn from(v: Model) -> Self {
        Self {
            hash: B256::from_slice(&v.hash),
            sender: Address::from_slice(&v.sender),
            nonce: B256::from_slice(&v.nonce),
            init_code: v.init_code.clone().map(Bytes::from),
            call_data: Bytes::from(v.call_data.clone()),
            call_gas_limit: U256::from(v.call_gas_limit.to_u128().unwrap_or(0)),
            verification_gas_limit: U256::from(v.verification_gas_limit.to_u128().unwrap_or(0)),
            pre_verification_gas: U256::from(v.pre_verification_gas.to_u128().unwrap_or(0)),
            max_fee_per_gas: U256::from(v.max_fee_per_gas.to_u128().unwrap_or(0)),
            max_priority_fee_per_gas: U256::from(v.max_priority_fee_per_gas.to_u128().unwrap_or(0)),
            paymaster_and_data: v.paymaster_and_data.clone().map(Bytes::from),
            signature: Bytes::from(v.signature.clone()),
            aggregator: v.aggregator.clone().map(|a| Address::from_slice(&a)),
            aggregator_signature: v.aggregator_signature.clone().map(Bytes::from),
            entry_point: Address::from_slice(&v.entry_point),
            entry_point_version: v.entry_point_version.clone(),
            transaction_hash: TxHash::from_slice(&v.transaction_hash),
            block_number: v.block_number as u64,
            block_hash: BlockHash::from_slice(&v.block_hash),
            bundler: Address::from_slice(&v.bundler),
            bundle_index: v.bundle_index as u32,
            index: v.index as u32,
            factory: v.factory.clone().map(|a| Address::from_slice(&a)),
            paymaster: v.paymaster.clone().map(|a| Address::from_slice(&a)),
            status: v.status,
            revert_reason: v.revert_reason.clone().map(Bytes::from),
            gas: U256::from(v.gas.to_u128().unwrap_or(0)),
            gas_price: U256::from(v.gas_price.to_u128().unwrap_or(0)),
            gas_used: U256::from(v.gas_used.to_u128().unwrap_or(0)),
            sponsor_type: v.sponsor_type.clone(),
            user_logs_start_index: v.user_logs_start_index as u32,
            user_logs_count: v.user_logs_count as u32,
            fee: U256::from(v.gas_price.mul(v.gas_used).to_u128().unwrap_or(0)),

            consensus: None,
            timestamp: None,
        }
    }
}

impl From<UserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
    fn from(v: UserOp) -> Self {
        let raw = match v.entry_point_version {
            EntryPointVersion::V06 => {
                user_ops_indexer_proto::blockscout::user_ops_indexer::v1::user_op::Raw::RawV06(
                    user_ops_indexer_proto::blockscout::user_ops_indexer::v1::RawUserOpV06 {
                        sender: v.sender.to_string(),
                        nonce: U256::from_be_slice(v.nonce.as_slice()).to_string(),
                        init_code: v.init_code.map_or("0x".to_string(), |b| b.to_string()),
                        call_data: v.call_data.to_string(),
                        call_gas_limit: v.call_gas_limit.to_string(),
                        verification_gas_limit: v.verification_gas_limit.to_string(),
                        pre_verification_gas: v.pre_verification_gas.to_string(),
                        max_fee_per_gas: v.max_fee_per_gas.to_string(),
                        max_priority_fee_per_gas: v.max_priority_fee_per_gas.to_string(),
                        paymaster_and_data: v
                            .paymaster_and_data
                            .map_or("0x".to_string(), |b| b.to_string()),
                        signature: v.signature.to_string(),
                    },
                )
            }
            EntryPointVersion::V07 => {
                user_ops_indexer_proto::blockscout::user_ops_indexer::v1::user_op::Raw::RawV07(
                    user_ops_indexer_proto::blockscout::user_ops_indexer::v1::RawUserOpV07 {
                        sender: v.sender.to_string(),
                        nonce: U256::from_be_slice(v.nonce.as_slice()).to_string(),
                        init_code: v.init_code.map_or("0x".to_string(), |b| b.to_string()),
                        call_data: v.call_data.to_string(),
                        account_gas_limits: B128::from(
                            v.verification_gas_limit.uint_try_to().unwrap_or(U128::ZERO),
                        )
                        .concat_const::<16, 32>(B128::from(
                            v.call_gas_limit.uint_try_to().unwrap_or(U128::ZERO),
                        ))
                        .to_string(),
                        pre_verification_gas: v.pre_verification_gas.to_string(),
                        gas_fees: B128::from(v.max_fee_per_gas.uint_try_to().unwrap_or(U128::ZERO))
                            .concat_const::<16, 32>(B128::from(
                                v.max_priority_fee_per_gas
                                    .uint_try_to()
                                    .unwrap_or(U128::ZERO),
                            ))
                            .to_string(),
                        paymaster_and_data: v
                            .paymaster_and_data
                            .map_or("0x".to_string(), |b| b.to_string()),
                        signature: v.signature.to_string(),
                    },
                )
            }
        };

        let (execute_target, execute_call_data) = decode_execute_call_data(&v.call_data);

        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
            hash: v.hash.to_string(),
            sender: v.sender.to_string(),
            nonce: v.nonce.to_string(),
            call_data: v.call_data.to_string(),
            call_gas_limit: v.call_gas_limit.to_string(),
            verification_gas_limit: v.verification_gas_limit.to_string(),
            pre_verification_gas: v.pre_verification_gas.to_string(),
            max_fee_per_gas: v.max_fee_per_gas.to_string(),
            max_priority_fee_per_gas: v.max_priority_fee_per_gas.to_string(),
            signature: v.signature.to_string(),
            raw: Some(raw),
            aggregator: v.aggregator.map(|a| a.to_string()),
            aggregator_signature: v.aggregator_signature.map(|b| b.to_string()),
            entry_point: v.entry_point.to_string(),
            entry_point_version: v.entry_point_version.to_value().to_string(),
            transaction_hash: v.transaction_hash.to_string(),
            block_number: v.block_number,
            block_hash: v.block_hash.to_string(),
            bundler: v.bundler.to_string(),
            bundle_index: v.bundle_index,
            index: v.index,
            factory: v.factory.map(|a| a.to_string()),
            paymaster: v.paymaster.map(|a| a.to_string()),
            status: v.status,
            revert_reason: v.revert_reason.map(|b| b.to_string()),
            gas: v.gas.to_string(),
            gas_price: v.gas_price.to_string(),
            gas_used: v.gas_used.to_string(),
            sponsor_type: v.sponsor_type.to_value().to_string(),
            user_logs_start_index: v.user_logs_start_index,
            user_logs_count: v.user_logs_count,
            fee: v.fee.to_string(),

            consensus: v.consensus,
            timestamp: v.timestamp,

            execute_target: execute_target.map(|a| a.to_string()),
            execute_call_data: execute_call_data.map(|b| b.to_string()),
        }
    }
}

impl From<ListUserOpDB> for ListUserOp {
    fn from(v: ListUserOpDB) -> Self {
        Self {
            hash: B256::from_slice(&v.hash),
            entry_point: Address::from_slice(&v.entry_point),
            entry_point_version: v.entry_point_version.clone(),
            block_number: v.block_number as u64,
            sender: Address::from_slice(&v.sender),
            transaction_hash: TxHash::from_slice(&v.transaction_hash),
            timestamp: v
                .timestamp
                .and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Micros, true),
            status: v.status,
            fee: U256::from(v.gas_price.mul(v.gas_used).to_u128().unwrap_or(0)),
        }
    }
}

impl From<ListUserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
    fn from(v: ListUserOp) -> Self {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
            hash: v.hash.to_string(),
            entry_point: v.entry_point.to_string(),
            entry_point_version: v.entry_point_version.to_value().to_string(),
            block_number: v.block_number,
            transaction_hash: v.transaction_hash.to_string(),
            address: v.sender.to_string(),
            timestamp: v.timestamp,
            status: v.status,
            fee: v.fee.to_string(),
        }
    }
}
