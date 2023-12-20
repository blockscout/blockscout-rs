use crate::{repository::user_op::ListUserOpDB, types::common::u256_to_decimal};
pub use entity::sea_orm_active_enums::SponsorType;
use entity::user_operations::Model;
use ethers::prelude::{Address, Bytes, H256, U256};
use ethers_core::{abi::AbiEncode, utils::to_checksum};
use num_traits::cast::ToPrimitive;
use sea_orm::{prelude::BigDecimal, ActiveEnum};
use std::ops::Mul;

#[derive(Clone, Debug, PartialEq)]
pub struct UserOp {
    pub hash: H256,
    pub sender: Address,
    pub nonce: H256,
    pub init_code: Option<Bytes>,
    pub call_data: Bytes,
    pub call_gas_limit: u64,
    pub verification_gas_limit: u64,
    pub pre_verification_gas: u64,
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
    pub paymaster_and_data: Option<Bytes>,
    pub signature: Bytes,
    pub aggregator: Option<Address>,
    pub aggregator_signature: Option<Bytes>,
    pub entry_point: Address,
    pub transaction_hash: H256,
    pub block_number: u64,
    pub block_hash: H256,
    pub bundler: Address,
    pub bundle_index: u64,
    pub index: u64,
    pub factory: Option<Address>,
    pub paymaster: Option<Address>,
    pub status: bool,
    pub revert_reason: Option<Bytes>,
    pub gas: u64,
    pub gas_price: U256,
    pub gas_used: u64,
    pub sponsor_type: SponsorType,
    pub user_logs_start_index: u64,
    pub user_logs_count: u64,
    pub fee: U256,

    pub consensus: Option<bool>,
    pub timestamp: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ListUserOp {
    pub hash: H256,
    pub block_number: u64,
    pub sender: Address,
    pub transaction_hash: H256,
    pub timestamp: u64,
    pub status: bool,
    pub fee: U256,
}

impl From<UserOp> for Model {
    fn from(v: UserOp) -> Self {
        Self {
            hash: v.hash.as_bytes().to_vec(),
            sender: v.sender.as_bytes().to_vec(),
            nonce: v.nonce.as_bytes().to_vec(),
            init_code: v.init_code.clone().map(|a| a.to_vec()),
            call_data: v.call_data.to_vec(),
            call_gas_limit: BigDecimal::from(v.call_gas_limit),
            verification_gas_limit: BigDecimal::from(v.verification_gas_limit),
            pre_verification_gas: BigDecimal::from(v.pre_verification_gas),
            max_fee_per_gas: u256_to_decimal(v.max_fee_per_gas),
            max_priority_fee_per_gas: u256_to_decimal(v.max_priority_fee_per_gas),
            paymaster_and_data: v.paymaster_and_data.clone().map(|a| a.to_vec()),
            signature: v.signature.to_vec(),
            aggregator: v.aggregator.map(|a| a.as_bytes().to_vec()),
            aggregator_signature: v.aggregator_signature.clone().map(|a| a.to_vec()),
            entry_point: v.entry_point.as_bytes().to_vec(),
            transaction_hash: v.transaction_hash.as_bytes().to_vec(),
            block_number: v.block_number as i32,
            block_hash: v.block_hash.as_bytes().to_vec(),
            bundler: v.bundler.as_bytes().to_vec(),
            bundle_index: v.bundle_index as i32,
            index: v.index as i32,
            factory: v.factory.map(|a| a.as_bytes().to_vec()),
            paymaster: v.paymaster.map(|a| a.as_bytes().to_vec()),
            status: v.status,
            revert_reason: v.revert_reason.clone().map(|a| a.to_vec()),
            gas: BigDecimal::from(v.gas),
            gas_price: u256_to_decimal(v.gas_price),
            gas_used: BigDecimal::from(v.gas_used),
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
            hash: H256::from_slice(&v.hash),
            sender: Address::from_slice(&v.sender),
            nonce: H256::from_slice(&v.nonce),
            init_code: v.init_code.clone().map(Bytes::from),
            call_data: Bytes::from(v.call_data.clone()),
            call_gas_limit: v.call_gas_limit.to_u64().unwrap_or(0),
            verification_gas_limit: v.verification_gas_limit.to_u64().unwrap_or(0),
            pre_verification_gas: v.pre_verification_gas.to_u64().unwrap_or(0),
            max_fee_per_gas: U256::from(v.max_fee_per_gas.to_u128().unwrap_or(0)),
            max_priority_fee_per_gas: U256::from(v.max_priority_fee_per_gas.to_u128().unwrap_or(0)),
            paymaster_and_data: v.paymaster_and_data.clone().map(Bytes::from),
            signature: Bytes::from(v.signature.clone()),
            aggregator: v.aggregator.clone().map(|a| Address::from_slice(&a)),
            aggregator_signature: v.aggregator_signature.clone().map(Bytes::from),
            entry_point: Address::from_slice(&v.entry_point),
            transaction_hash: H256::from_slice(&v.transaction_hash),
            block_number: v.block_number as u64,
            block_hash: H256::from_slice(&v.block_hash),
            bundler: Address::from_slice(&v.bundler),
            bundle_index: v.bundle_index as u64,
            index: v.index as u64,
            factory: v.factory.clone().map(|a| Address::from_slice(&a)),
            paymaster: v.paymaster.clone().map(|a| Address::from_slice(&a)),
            status: v.status,
            revert_reason: v.revert_reason.clone().map(Bytes::from),
            gas: v.gas.to_u64().unwrap_or(0),
            gas_price: U256::from(v.gas_price.to_u128().unwrap_or(0)),
            gas_used: v.gas_used.to_u64().unwrap_or(0),
            sponsor_type: v.sponsor_type.clone(),
            user_logs_start_index: v.user_logs_start_index as u64,
            user_logs_count: v.user_logs_count as u64,
            fee: U256::from(v.gas_price.mul(v.gas_used).to_u128().unwrap_or(0)),

            consensus: None,
            timestamp: None,
        }
    }
}

impl From<UserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
    fn from(v: UserOp) -> Self {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
            hash: v.hash.encode_hex(),
            sender: to_checksum(&v.sender, None),
            nonce: v.nonce.encode_hex(),
            init_code: v.init_code.map(|b| b.to_string()),
            call_data: v.call_data.to_string(),
            call_gas_limit: v.call_gas_limit,
            verification_gas_limit: v.verification_gas_limit,
            pre_verification_gas: v.pre_verification_gas,
            max_fee_per_gas: v.max_fee_per_gas.to_string(),
            max_priority_fee_per_gas: v.max_priority_fee_per_gas.to_string(),
            paymaster_and_data: v.paymaster_and_data.map(|b| b.to_string()),
            signature: v.signature.to_string(),
            aggregator: v.aggregator.map(|a| to_checksum(&a, None)),
            aggregator_signature: v.aggregator_signature.map(|b| b.to_string()),
            entry_point: to_checksum(&v.entry_point, None),
            transaction_hash: v.transaction_hash.encode_hex(),
            block_number: v.block_number,
            block_hash: v.block_hash.encode_hex(),
            bundler: to_checksum(&v.bundler, None),
            bundle_index: v.bundle_index,
            index: v.index,
            factory: v.factory.map(|a| to_checksum(&a, None)),
            paymaster: v.paymaster.map(|a| to_checksum(&a, None)),
            status: v.status,
            revert_reason: v.revert_reason.map(|b| b.to_string()),
            gas: v.gas,
            gas_price: v.gas_price.to_string(),
            gas_used: v.gas_used,
            sponsor_type: v.sponsor_type.to_value().to_string(),
            user_logs_start_index: v.user_logs_start_index,
            user_logs_count: v.user_logs_count,
            fee: v.fee.to_string(),

            consensus: v.consensus,
            timestamp: v.timestamp,
        }
    }
}

impl From<ListUserOpDB> for ListUserOp {
    fn from(v: ListUserOpDB) -> Self {
        Self {
            hash: H256::from_slice(&v.hash),
            block_number: v.block_number as u64,
            sender: Address::from_slice(&v.sender),
            transaction_hash: H256::from_slice(&v.transaction_hash),
            timestamp: v.timestamp.timestamp() as u64,
            status: v.status,
            fee: U256::from(v.gas_price.mul(v.gas_used).to_u128().unwrap_or(0)),
        }
    }
}

impl From<ListUserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
    fn from(v: ListUserOp) -> Self {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
            hash: v.hash.encode_hex(),
            block_number: v.block_number,
            transaction_hash: v.transaction_hash.encode_hex(),
            address: to_checksum(&v.sender, None),
            timestamp: v.timestamp,
            status: v.status,
            fee: v.fee.to_string(),
        }
    }
}
