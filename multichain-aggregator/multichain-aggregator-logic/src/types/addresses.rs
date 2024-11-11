use super::ChainId;
use crate::{error::ParseError, proto};
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

impl TryFrom<Model> for Address {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id: v.chain_id,
            hash: alloy_primitives::Address::try_from(v.hash.as_slice())?,
            ens_name: v.ens_name,
            contract_name: v.contract_name,
            token_name: v.token_name,
            token_type: v.token_type,
            is_contract: v.is_contract,
            is_verified_contract: v.is_verified_contract,
            is_token: v.is_token,
        })
    }
}

impl From<Address> for proto::Address {
    fn from(v: Address) -> Self {
        Self {
            hash: v.hash.to_string(),
            ens_name: v.ens_name,
            contract_name: v.contract_name,
            token_name: v.token_name,
            token_type: v
                .token_type
                .map(db_token_type_to_proto_token_type)
                .unwrap_or_default()
                .into(),
            is_contract: Some(v.is_contract),
            is_verified_contract: Some(v.is_verified_contract),
            is_token: Some(v.is_token),
        }
    }
}

pub fn proto_token_type_to_db_token_type(
    token_type: proto::address::TokenType,
) -> Option<db_enum::TokenType> {
    match token_type {
        proto::address::TokenType::Erc20 => Some(db_enum::TokenType::Erc20),
        proto::address::TokenType::Erc1155 => Some(db_enum::TokenType::Erc1155),
        proto::address::TokenType::Erc721 => Some(db_enum::TokenType::Erc721),
        proto::address::TokenType::Erc404 => Some(db_enum::TokenType::Erc404),
        proto::address::TokenType::Unspecified => None,
    }
}

pub fn db_token_type_to_proto_token_type(
    token_type: db_enum::TokenType,
) -> proto::address::TokenType {
    match token_type {
        db_enum::TokenType::Erc20 => proto::address::TokenType::Erc20,
        db_enum::TokenType::Erc1155 => proto::address::TokenType::Erc1155,
        db_enum::TokenType::Erc721 => proto::address::TokenType::Erc721,
        db_enum::TokenType::Erc404 => proto::address::TokenType::Erc404,
    }
}
