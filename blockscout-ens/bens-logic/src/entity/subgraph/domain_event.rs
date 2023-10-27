use ethers::types::{Address, TxHash};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, sqlx::FromRow)]
pub struct DomainEventTransaction {
    pub block_number: i32,
    pub transaction_id: Vec<u8>,
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainEvent {
    pub transaction_hash: TxHash,
    pub block_number: i64,
    pub timestamp: String,
    pub from_address: Address,
    pub method: Option<String>,
    pub actions: Vec<String>,
}
