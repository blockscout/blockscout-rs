use super::ChainId;
use entity::{addresses::Model, sea_orm_active_enums as db_enum};

#[derive(Debug, Clone)]
pub struct Address {
    pub chain_id: ChainId,
    pub hash: alloy_primitives::Address,
    pub ens_name: Option<String>,
    pub contract_name: Option<String>,
    pub token_name: Option<String>,
    pub token_type: Option<db_enum::TokenType>,
    pub is_contract: bool,
    pub is_verified_contract: bool,
    pub is_token: bool,
}

impl From<Address> for Model {
    fn from(v: Address) -> Self {
        Self {
            chain_id: v.chain_id,
            hash: v.hash.to_vec(),
            ens_name: v.ens_name,
            contract_name: v.contract_name,
            token_name: v.token_name,
            token_type: v.token_type,
            is_contract: v.is_contract,
            is_verified_contract: v.is_verified_contract,
            is_token: v.is_token,
            created_at: Default::default(),
            updated_at: Default::default(),
        }
    }
}
