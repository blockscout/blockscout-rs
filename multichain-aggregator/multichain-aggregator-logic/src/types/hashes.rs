use super::ChainId;
use entity::{hashes::Model, sea_orm_active_enums as db_enum};

#[derive(Debug, Clone)]
pub struct Hash {
    pub chain_id: ChainId,
    pub hash: alloy_primitives::B256,
    pub hash_type: db_enum::HashType,
}

impl From<Hash> for Model {
    fn from(v: Hash) -> Self {
        Self {
            hash: v.hash.to_vec(),
            chain_id: v.chain_id,
            hash_type: v.hash_type,
            created_at: Default::default(),
        }
    }
}
