use ethers::prelude::{Address, Bytes, H256, U256};
use ethers_core::abi::AbiEncode;
use ethers_core::utils::to_checksum;
use num_traits::cast::ToPrimitive;
use sea_orm::prelude::Decimal;
use sea_orm::ActiveEnum;

pub use entity::sea_orm_active_enums::SponsorType;
use entity::user_operations::Model;

use crate::repository::user_op::ListUserOpDB;
use crate::types::common::u256_to_decimal;

#[derive(Clone)]
pub struct UserOp {
    pub op_hash: H256,
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
    pub tx_hash: H256,
    pub block_number: u64,
    pub block_hash: H256,
    pub bundler: Address,
    pub bundle_index: u64,
    pub op_index: u64,
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

    pub consensus: Option<bool>,
    pub timestamp: Option<u64>,
}

#[derive(Clone)]
pub struct ListUserOp {
    pub op_hash: H256,
    pub block_number: u64,
    pub sender: Address,
    pub tx_hash: H256,
    pub timestamp: u64,
}

impl From<UserOp> for Model {
    fn from(v: UserOp) -> Self {
        Self {
            op_hash: v.op_hash.as_bytes().to_vec(),
            sender: v.sender.as_bytes().to_vec(),
            nonce: v.nonce.as_bytes().to_vec(),
            init_code: v.init_code.clone().map(|a| a.to_vec()),
            call_data: v.call_data.to_vec(),
            call_gas_limit: Decimal::from(v.call_gas_limit),
            verification_gas_limit: Decimal::from(v.verification_gas_limit),
            pre_verification_gas: Decimal::from(v.pre_verification_gas),
            max_fee_per_gas: u256_to_decimal(v.max_fee_per_gas),
            max_priority_fee_per_gas: u256_to_decimal(v.max_priority_fee_per_gas),
            paymaster_and_data: v.paymaster_and_data.clone().map(|a| a.to_vec()),
            signature: v.signature.to_vec(),
            aggregator: v.aggregator.map(|a| a.as_bytes().to_vec()),
            aggregator_signature: v.aggregator_signature.clone().map(|a| a.to_vec()),
            entry_point: v.entry_point.as_bytes().to_vec(),
            tx_hash: v.tx_hash.as_bytes().to_vec(),
            block_number: v.block_number as i32,
            block_hash: v.block_hash.as_bytes().to_vec(),
            bundler: v.bundler.as_bytes().to_vec(),
            bundle_index: v.bundle_index as i32,
            op_index: v.op_index as i32,
            factory: v.factory.map(|a| a.as_bytes().to_vec()),
            paymaster: v.paymaster.map(|a| a.as_bytes().to_vec()),
            status: v.status,
            revert_reason: v.revert_reason.clone().map(|a| a.to_vec()),
            gas: Decimal::from(v.gas),
            gas_price: u256_to_decimal(v.gas_price),
            gas_used: Decimal::from(v.gas_used),
            sponsor_type: v.sponsor_type.clone(),
            user_logs_start_index: v.user_logs_start_index as i32,
            user_logs_count: v.user_logs_count as i32,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}

impl From<Model> for UserOp {
    fn from(v: Model) -> Self {
        Self {
            op_hash: H256::from_slice(&v.op_hash),
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
            tx_hash: H256::from_slice(&v.tx_hash),
            block_number: v.block_number as u64,
            block_hash: H256::from_slice(&v.block_hash),
            bundler: Address::from_slice(&v.bundler),
            bundle_index: v.bundle_index as u64,
            op_index: v.op_index as u64,
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

            consensus: None,
            timestamp: None,
        }
    }
}

impl From<UserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
    fn from(v: UserOp) -> Self {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::UserOp {
            op_hash: v.op_hash.encode_hex(),
            sender: to_checksum(&v.sender, None),
            nonce: v.nonce.encode_hex(),
            init_code: v.init_code.map(|b| bytes::Bytes::from(b.to_vec())),
            call_data: bytes::Bytes::from(v.call_data.to_vec()),
            call_gas_limit: v.call_gas_limit,
            verification_gas_limit: v.verification_gas_limit,
            pre_verification_gas: v.pre_verification_gas,
            max_fee_per_gas: v.max_fee_per_gas.to_string(),
            max_priority_fee_per_gas: v.max_priority_fee_per_gas.to_string(),
            paymaster_and_data: v.paymaster_and_data.map(|b| bytes::Bytes::from(b.to_vec())),
            signature: bytes::Bytes::from(v.signature.to_vec()),
            aggregator: v.aggregator.map(|a| to_checksum(&a, None)),
            aggregator_signature: v
                .aggregator_signature
                .map(|b| bytes::Bytes::from(b.to_vec())),
            entry_point: to_checksum(&v.entry_point, None),
            tx_hash: v.tx_hash.encode_hex(),
            block_number: v.block_number,
            block_hash: v.block_hash.encode_hex(),
            bundler: to_checksum(&v.bundler, None),
            bundle_index: v.bundle_index,
            op_index: v.op_index,
            factory: v.factory.map(|a| to_checksum(&a, None)),
            paymaster: v.paymaster.map(|a| to_checksum(&a, None)),
            status: v.status,
            revert_reason: v.revert_reason.map(|b| bytes::Bytes::from(b.to_vec())),
            gas: v.gas,
            gas_price: v.gas_price.to_string(),
            gas_used: v.gas_used,
            sponsor_type: v.sponsor_type.to_value().to_string(),
            user_logs_start_index: v.user_logs_start_index,
            user_logs_count: v.user_logs_count,

            consensus: v.consensus,
            timestamp: v.timestamp,
        }
    }
}

impl From<ListUserOpDB> for ListUserOp {
    fn from(v: ListUserOpDB) -> Self {
        Self {
            op_hash: H256::from_slice(&v.op_hash),
            block_number: v.block_number as u64,
            sender: Address::from_slice(&v.sender),
            tx_hash: H256::from_slice(&v.tx_hash),
            timestamp: v.timestamp.timestamp() as u64,
        }
    }
}

impl From<ListUserOp> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
    fn from(v: ListUserOp) -> Self {
        user_ops_indexer_proto::blockscout::user_ops_indexer::v1::ListUserOp {
            op_hash: v.op_hash.encode_hex(),
            block_number: v.block_number,
            tx_hash: v.tx_hash.encode_hex(),
            address: to_checksum(&v.sender, None),
            timestamp: v.timestamp,
        }
    }
}
