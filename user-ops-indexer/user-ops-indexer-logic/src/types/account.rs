use crate::repository::account::AccountDB;
pub use entity::sea_orm_active_enums::SponsorType;
use ethers::prelude::{Address, H256};
use ethers_core::{abi::AbiEncode, utils::to_checksum};

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub address: Address,
    pub factory: Option<Address>,
    pub creation_transaction_hash: Option<H256>,
    pub creation_op_hash: Option<H256>,
    pub creation_timestamp: Option<u64>,
    pub total_ops: u32,
}

impl From<AccountDB> for Account {
    fn from(v: AccountDB) -> Self {
        Self {
            address: Address::from_slice(&v.address),
            factory: v.factory.map(|a| Address::from_slice(&a)),
            creation_transaction_hash: v.creation_transaction_hash.map(|a| H256::from_slice(&a)),
            creation_op_hash: v.creation_op_hash.map(|a| H256::from_slice(&a)),
            creation_timestamp: v.creation_timestamp.map(|t| t.timestamp() as u64),
            total_ops: v.total_ops as u32,
        }
    }
}

impl From<Account> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Account {
    fn from(v: Account) -> Self {
        Self {
            address: to_checksum(&v.address, None),
            factory: v.factory.map(|a| to_checksum(&a, None)),
            creation_transaction_hash: v.creation_transaction_hash.map(|a| a.encode_hex()),
            creation_op_hash: v.creation_op_hash.map(|a| a.encode_hex()),
            creation_timestamp: v.creation_timestamp,
            total_ops: v.total_ops,
        }
    }
}
