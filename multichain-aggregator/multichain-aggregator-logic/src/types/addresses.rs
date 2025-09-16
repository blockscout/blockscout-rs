use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{domains::DomainInfo, sea_orm_wrappers::SeaOrmAddress},
};
use bigdecimal::BigDecimal;
use entity::{
    address_coin_balances,
    addresses::{Column, Entity, Model},
};
use sea_orm::{
    DerivePartialModel, FromJsonQueryResult, IntoSimpleExpr,
    prelude::Expr,
    sea_query::{Func, SimpleExpr},
};
use serde::{Deserialize, Serialize};

pub type TokenType = entity::sea_orm_active_enums::TokenType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    pub chain_id: ChainId,
    pub hash: alloy_primitives::Address,
    pub domain_info: Option<DomainInfo>,
    pub contract_name: Option<String>,
    pub token_name: Option<String>,
    pub token_type: Option<TokenType>,
    pub is_contract: bool,
    pub is_verified_contract: bool,
    pub is_token: bool,
}

impl From<Address> for Model {
    fn from(v: Address) -> Self {
        Self {
            chain_id: v.chain_id,
            hash: v.hash.to_vec(),
            ens_name: None,
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
            domain_info: None,
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
            domain_info: v.domain_info.map(|d| d.into()),
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
            chain_id: v.chain_id.to_string(),
        }
    }
}

fn chain_info_query() -> SimpleExpr {
    Expr::cust_with_exprs(
        "json_build_object('chain_id',$1,'coin_balance',$2,'is_contract',$3,'is_verified',$4)",
        vec![
            Column::ChainId.into_simple_expr(),
            Func::coalesce([
                address_coin_balances::Column::Value.into_simple_expr(),
                Expr::value(0),
            ])
            .into(),
            Column::IsContract.into_simple_expr(),
            Column::IsVerifiedContract.into_simple_expr(),
        ],
    )
    .into_simple_expr()
}

fn chain_infos_query() -> SimpleExpr {
    Expr::cust_with_expr("json_agg($1)", chain_info_query().into_simple_expr())
}

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct AggregatedAddressInfo {
    pub hash: SeaOrmAddress,
    #[sea_orm(from_expr = r#"chain_infos_query()"#)]
    pub chain_infos: Vec<ChainInfo>,
    #[sea_orm(skip)]
    pub has_tokens: bool,
    #[sea_orm(skip)]
    pub has_interop_message_transfers: bool,
    #[sea_orm(skip)]
    pub domain_info: Option<DomainInfo>,
}

impl AggregatedAddressInfo {
    pub fn default(hash: SeaOrmAddress) -> Self {
        Self {
            hash,
            chain_infos: vec![],
            has_tokens: false,
            has_interop_message_transfers: false,
            domain_info: None,
        }
    }
}

impl From<AggregatedAddressInfo> for proto::GetAddressResponse {
    fn from(v: AggregatedAddressInfo) -> Self {
        Self {
            hash: v.hash.to_string(),
            chain_infos: v
                .chain_infos
                .into_iter()
                .map(|c| (c.chain_id.to_string(), c.into()))
                .collect(),
            has_tokens: v.has_tokens,
            has_interop_message_transfers: v.has_interop_message_transfers,
        }
    }
}

impl TryFrom<AggregatedAddressInfo> for proto::Address {
    type Error = ParseError;

    fn try_from(v: AggregatedAddressInfo) -> Result<Self, Self::Error> {
        let chain_id = match v.chain_infos.as_slice() {
            [chain_info] => chain_info.chain_id.to_string(),
            _ => {
                return Err(ParseError::Custom(
                    "address info should have exactly one chain info".to_string(),
                ));
            }
        };

        Ok(Self {
            hash: v.hash.to_string(),
            domain_info: v.domain_info.map(|d| d.into()),
            chain_id,
            ..Default::default()
        })
    }
}

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct ChainAddressInfo {
    pub hash: SeaOrmAddress,
    #[sea_orm(from_expr = r#"chain_info_query()"#)]
    pub chain_info: ChainInfo,
    #[sea_orm(skip)]
    pub domain_info: Option<DomainInfo>,
}

impl From<ChainAddressInfo> for proto::Address {
    fn from(v: ChainAddressInfo) -> Self {
        let chain_id = v.chain_info.chain_id.to_string();

        Self {
            hash: v.hash.to_string(),
            domain_info: v.domain_info.map(|d| d.into()),
            chain_id,
            ..Default::default()
        }
    }
}

#[derive(Debug, FromJsonQueryResult, Serialize, Deserialize, Clone)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub coin_balance: BigDecimal,
    is_contract: bool,
    is_verified: bool,
}

impl From<ChainInfo> for proto::get_address_response::ChainInfo {
    fn from(v: ChainInfo) -> Self {
        Self {
            coin_balance: v.coin_balance.to_string(),
            is_contract: v.is_contract,
            is_verified: v.is_verified,
        }
    }
}

pub fn proto_token_type_to_db_token_type(token_type: proto::TokenType) -> Option<TokenType> {
    match token_type {
        proto::TokenType::Erc20 => Some(TokenType::Erc20),
        proto::TokenType::Erc1155 => Some(TokenType::Erc1155),
        proto::TokenType::Erc721 => Some(TokenType::Erc721),
        proto::TokenType::Erc404 => Some(TokenType::Erc404),
        proto::TokenType::Erc7802 => Some(TokenType::Erc7802),
        proto::TokenType::Unspecified => None,
    }
}

pub fn db_token_type_to_proto_token_type(token_type: TokenType) -> proto::TokenType {
    match token_type {
        TokenType::Erc20 => proto::TokenType::Erc20,
        TokenType::Erc1155 => proto::TokenType::Erc1155,
        TokenType::Erc721 => proto::TokenType::Erc721,
        TokenType::Erc404 => proto::TokenType::Erc404,
        TokenType::Erc7802 => proto::TokenType::Erc7802,
    }
}
