use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{
        domains::{BasicDomainInfo, DomainInfo},
        sea_orm_wrappers::SeaOrmAddress,
    },
};
use bigdecimal::BigDecimal;
use entity::{
    address_coin_balances,
    addresses::{ActiveModel, Column, Entity, Model},
};
use sea_orm::{
    ActiveValue::Set,
    DerivePartialModel, FromJsonQueryResult, IntoSimpleExpr,
    prelude::{DateTime, Expr},
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
    pub is_contract: bool,
    pub is_verified_contract: bool,
}

impl From<Address> for ActiveModel {
    fn from(v: Address) -> Self {
        Self {
            chain_id: Set(v.chain_id),
            hash: Set(v.hash.to_vec()),
            contract_name: Set(v.contract_name),
            is_contract: Set(v.is_contract),
            is_verified_contract: Set(v.is_verified_contract),
            ..Default::default()
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
            is_contract: v.is_contract,
            is_verified_contract: v.is_verified_contract,
        })
    }
}

impl From<Address> for proto::Address {
    fn from(v: Address) -> Self {
        Self {
            hash: v.hash.to_string(),
            domain_info: v.domain_info.map(|d| d.into()),
            contract_name: v.contract_name,
            token_name: None,
            token_type: proto::TokenType::Unspecified.into(),
            is_contract: Some(v.is_contract),
            is_verified_contract: Some(v.is_verified_contract),
            is_token: None,
            chain_id: v.chain_id.to_string(),
        }
    }
}

fn chain_info_query() -> SimpleExpr {
    Expr::cust_with_exprs(
        "json_build_object('chain_id',$1,'coin_balance',$2,'is_contract',$3,'is_verified',$4,'contract_name',$5)",
        vec![
            Column::ChainId.into_simple_expr(),
            Func::coalesce([
                address_coin_balances::Column::Value.into_simple_expr(),
                Expr::value(0),
            ])
            .into(),
            Column::IsContract.into_simple_expr(),
            Column::IsVerifiedContract.into_simple_expr(),
            Column::ContractName.into_simple_expr(),
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
    #[sea_orm(skip)]
    pub exchange_rate: Option<String>,
}

impl AggregatedAddressInfo {
    pub fn default(hash: SeaOrmAddress) -> Self {
        Self {
            hash,
            chain_infos: vec![],
            has_tokens: false,
            has_interop_message_transfers: false,
            domain_info: None,
            exchange_rate: None,
        }
    }
}

impl From<AggregatedAddressInfo> for proto::GetAddressResponse {
    fn from(v: AggregatedAddressInfo) -> Self {
        let coin_balance = v
            .chain_infos
            .iter()
            .map(|c| c.coin_balance.clone())
            .sum::<BigDecimal>()
            .to_string();
        Self {
            hash: v.hash.to_string(),
            chain_infos: v
                .chain_infos
                .into_iter()
                .map(|c| (c.chain_id.to_string(), c.into()))
                .collect(),
            has_tokens: v.has_tokens,
            has_interop_message_transfers: v.has_interop_message_transfers,
            coin_balance,
            exchange_rate: v.exchange_rate,
            domains: v
                .domain_info
                .map(BasicDomainInfo::from)
                .map(|d| vec![d.into()])
                .unwrap_or_default(),
        }
    }
}

impl TryFrom<AggregatedAddressInfo> for proto::Address {
    type Error = ParseError;

    fn try_from(v: AggregatedAddressInfo) -> Result<Self, Self::Error> {
        let chain_info = match v.chain_infos.as_slice() {
            [chain_info] => chain_info,
            _ => {
                return Err(ParseError::Custom(
                    "address info should have exactly one chain info".to_string(),
                ));
            }
        };

        Ok(Self {
            hash: v.hash.to_string(),
            domain_info: v.domain_info.map(|d| d.into()),
            chain_id: chain_info.chain_id.to_string(),
            contract_name: chain_info.contract_name.clone(),
            is_contract: Some(chain_info.is_contract),
            is_verified_contract: Some(chain_info.is_verified),
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
        let chain_info = v.chain_info;

        Self {
            hash: v.hash.to_string(),
            domain_info: v.domain_info.map(|d| d.into()),
            chain_id: chain_info.chain_id.to_string(),
            contract_name: chain_info.contract_name.clone(),
            is_contract: Some(chain_info.is_contract),
            is_verified_contract: Some(chain_info.is_verified),
            ..Default::default()
        }
    }
}

#[derive(Debug, FromJsonQueryResult, Serialize, Deserialize, Clone)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub coin_balance: BigDecimal,
    pub is_contract: bool,
    pub is_verified: bool,
    pub contract_name: Option<String>,
}

impl From<ChainInfo> for proto::get_address_response::ChainInfo {
    fn from(v: ChainInfo) -> Self {
        Self {
            coin_balance: v.coin_balance.to_string(),
            is_contract: v.is_contract,
            is_verified: v.is_verified,
            contract_name: v.contract_name,
        }
    }
}

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct StreamAddressUpdate {
    pub hash: SeaOrmAddress,
    pub chain_id: ChainId,
    pub updated_at: DateTime,
    pub contract_name: Option<String>,
}

impl From<StreamAddressUpdate> for proto::stream_address_updates_response::StreamAddressUpdate {
    fn from(v: StreamAddressUpdate) -> Self {
        Self {
            hash: v.hash.to_string(),
            chain_id: v.chain_id.to_string(),
            updated_at: v.updated_at.and_utc().timestamp_micros(),
            contract_name: v.contract_name,
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
