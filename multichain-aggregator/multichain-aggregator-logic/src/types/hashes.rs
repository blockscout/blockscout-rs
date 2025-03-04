use super::ChainId;
use crate::{error::ParseError, proto};
use entity::hashes::Model;

pub type HashType = entity::sea_orm_active_enums::HashType;

#[derive(Debug, Clone)]
pub struct Hash {
    pub chain_id: ChainId,
    pub hash: alloy_primitives::B256,
    pub hash_type: HashType,
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

impl TryFrom<Model> for Hash {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: v.chain_id,
            hash: alloy_primitives::B256::try_from(v.hash.as_slice())?,
            hash_type: v.hash_type,
        })
    }
}

impl From<Hash> for proto::Hash {
    fn from(v: Hash) -> Self {
        Self {
            hash: v.hash.to_string(),
            hash_type: db_hash_type_to_proto_hash_type(v.hash_type).into(),
            chain_id: v.chain_id.to_string(),
        }
    }
}

pub fn proto_hash_type_to_db_hash_type(hash_type: proto::HashType) -> HashType {
    match hash_type {
        proto::HashType::Block => HashType::Block,
        proto::HashType::Transaction => HashType::Transaction,
    }
}

pub fn db_hash_type_to_proto_hash_type(hash_type: HashType) -> proto::HashType {
    match hash_type {
        HashType::Block => proto::HashType::Block,
        HashType::Transaction => proto::HashType::Transaction,
    }
}
