use crate::repository::account::AccountDB;
use alloy::primitives::{Address, TxHash, B256};

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub address: Address,
    pub factory: Option<Address>,
    pub creation_transaction_hash: Option<TxHash>,
    pub creation_op_hash: Option<B256>,
    pub creation_timestamp: Option<String>,
    pub total_ops: u32,
}

impl From<AccountDB> for Account {
    fn from(v: AccountDB) -> Self {
        Self {
            address: Address::from_slice(&v.address),
            factory: v.factory.map(|a| Address::from_slice(&a)),
            creation_transaction_hash: v.creation_transaction_hash.map(|a| TxHash::from_slice(&a)),
            creation_op_hash: v.creation_op_hash.map(|a| B256::from_slice(&a)),
            creation_timestamp: v.creation_timestamp.map(|t| {
                t.and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Micros, true)
            }),
            total_ops: v.total_ops as u32,
        }
    }
}

impl From<Account> for user_ops_indexer_proto::blockscout::user_ops_indexer::v1::Account {
    fn from(v: Account) -> Self {
        Self {
            address: v.address.to_string(),
            factory: v.factory.map(|a| a.to_string()),
            creation_transaction_hash: v.creation_transaction_hash.map(|a| a.to_string()),
            creation_op_hash: v.creation_op_hash.map(|a| a.to_string()),
            creation_timestamp: v.creation_timestamp,
            total_ops: v.total_ops,
        }
    }
}
