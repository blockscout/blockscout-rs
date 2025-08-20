use std::vec::Vec;

#[derive(Debug, Clone)]
pub struct ContractRecord {
    pub id: i64,
    pub chain_id: String,
    pub address_db: Vec<u8>,
    pub blockscout_url: String,
}
