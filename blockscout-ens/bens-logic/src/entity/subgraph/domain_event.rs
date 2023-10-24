use ethers::types::Address;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct DomainEventTransaction {
    pub block_number: i32,
    pub transaction_id: Vec<u8>,
}

pub struct DomainEvent {
    pub transaction_hash: Vec<u8>,
    pub timestamp: i64,
    pub from_address: Address,
    pub action: String,
}
