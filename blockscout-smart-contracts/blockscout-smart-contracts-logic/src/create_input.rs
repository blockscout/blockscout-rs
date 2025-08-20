use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct CreateInput {
    pub chain_id: String,
    pub address_bytes: Vec<u8>,
    pub blockscout_url: String,
    pub sources: BTreeMap<String, String>,
}
