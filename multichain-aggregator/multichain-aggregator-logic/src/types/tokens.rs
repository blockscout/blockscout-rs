use super::ChainId;
use crate::{
    clients::blockscout::{node_api_config::NodeApiConfigResponse, stats::StatsResponse},
    error::ParseError,
    proto,
    types::{
        addresses::db_token_type_to_proto_token_type, macros::opt_parse,
        sea_orm_wrappers::SeaOrmAddress,
    },
};
use entity::tokens::{ActiveModel, Column, Entity};
use sea_orm::{
    ActiveValue::{NotSet, Set, Unchanged},
    DeriveIntoActiveModel, DerivePartialModel, FromJsonQueryResult, IntoActiveModel,
    IntoActiveValue, IntoSimpleExpr,
    prelude::{BigDecimal, ColumnTrait, DateTime, Decimal, Expr},
    sea_query::SimpleExpr,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type TokenType = entity::sea_orm_active_enums::TokenType;
pub const NATIVE_TOKEN_ADDRESS: alloy_primitives::Address = alloy_primitives::Address::ZERO;

#[derive(Debug, Clone, Default)]
pub struct TokenUpdate {
    pub metadata: Option<UpdateTokenMetadata>,
    pub price_data: Option<UpdateTokenPriceData>,
    pub counters: Option<UpdateTokenCounters>,
    pub r#type: Option<UpdateTokenType>,
}

impl TryFrom<(ChainId, NodeApiConfigResponse)> for TokenUpdate {
    type Error = ParseError;

    fn try_from((chain_id, config): (ChainId, NodeApiConfigResponse)) -> Result<Self, Self::Error> {
        Ok(Self {
            metadata: Some(UpdateTokenMetadata {
                chain_id,
                address_hash: NATIVE_TOKEN_ADDRESS.to_vec(),
                name: config.envs.next_public_network_currency_name,
                symbol: config.envs.next_public_network_currency_symbol,
                decimals: opt_parse!(config.envs.next_public_network_currency_decimals),
                token_type: Some(TokenType::Native),
                ..Default::default()
            }),
            ..Default::default()
        })
    }
}

impl TryFrom<(ChainId, StatsResponse)> for TokenUpdate {
    type Error = ParseError;

    fn try_from((chain_id, stats): (ChainId, StatsResponse)) -> Result<Self, Self::Error> {
        Ok(Self {
            metadata: Some(UpdateTokenMetadata {
                chain_id,
                address_hash: NATIVE_TOKEN_ADDRESS.to_vec(),
                token_type: Some(TokenType::Native),
                icon_url: stats.coin_image,
                ..Default::default()
            }),
            price_data: Some(UpdateTokenPriceData {
                chain_id,
                address_hash: NATIVE_TOKEN_ADDRESS.to_vec(),
                fiat_value: opt_parse!(stats.coin_price),
                circulating_market_cap: opt_parse!(stats.market_cap),
            }),
            ..Default::default()
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct UpdateTokenMetadata {
    pub chain_id: ChainId,
    pub address_hash: Vec<u8>,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<i16>,
    pub token_type: Option<TokenType>,
    pub icon_url: Option<String>,
    pub total_supply: Option<BigDecimal>,
}

// Manually implement IntoActiveModel because IntoActiveValue is not implemented
// for BigDecimal and TokenType enum
impl IntoActiveModel<ActiveModel> for UpdateTokenMetadata {
    fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            chain_id: IntoActiveValue::<_>::into_active_value(self.chain_id),
            address_hash: IntoActiveValue::<_>::into_active_value(self.address_hash),
            name: IntoActiveValue::<_>::into_active_value(self.name),
            symbol: IntoActiveValue::<_>::into_active_value(self.symbol),
            decimals: IntoActiveValue::<_>::into_active_value(self.decimals),
            token_type: match self.token_type {
                Some(value) => Set(value),
                None => NotSet,
            },
            icon_url: IntoActiveValue::<_>::into_active_value(self.icon_url),
            total_supply: Set(self.total_supply),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, DeriveIntoActiveModel)]
pub struct UpdateTokenPriceData {
    pub chain_id: ChainId,
    pub address_hash: Vec<u8>,
    pub fiat_value: Option<Decimal>,
    pub circulating_market_cap: Option<Decimal>,
}

#[derive(Debug, Clone, DeriveIntoActiveModel)]
pub struct UpdateTokenCounters {
    pub chain_id: ChainId,
    pub address_hash: Vec<u8>,
    pub holders_count: Option<i64>,
    pub transfers_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct UpdateTokenType {
    pub chain_id: ChainId,
    pub address_hash: Vec<u8>,
    pub token_type: TokenType,
}

// Manually implement IntoActiveModel because IntoActiveValue is not implemented
// for TokenType enum
impl IntoActiveModel<ActiveModel> for UpdateTokenType {
    fn into_active_model(self) -> ActiveModel {
        ActiveModel {
            chain_id: Unchanged(self.chain_id),
            address_hash: Unchanged(self.address_hash),
            token_type: Set(self.token_type),
            ..Default::default()
        }
    }
}

pub fn token_chain_infos_expr() -> SimpleExpr {
    Expr::cust_with_exprs(
        "jsonb_build_object('chain_id',$1,'holders_count',$2,'total_supply',$3,'is_verified_contract',$4,'contract_name',$5)",
        [
            Column::ChainId.into_simple_expr(),
            Column::HoldersCount.into_simple_expr(),
            Column::TotalSupply.into_simple_expr(),
            entity::addresses::Column::IsVerifiedContract
                .if_null(false)
                .into_simple_expr(),
            entity::addresses::Column::ContractName.into_simple_expr(),
        ],
    )
}

#[derive(Debug, Clone, DerivePartialModel, Serialize, Deserialize)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct AggregatedToken {
    pub address_hash: SeaOrmAddress,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub decimals: Option<i16>,
    pub token_type: TokenType,
    pub icon_url: Option<String>,
    pub fiat_value: Option<Decimal>,
    pub circulating_market_cap: Option<Decimal>,
    pub total_supply: Option<BigDecimal>,
    pub holders_count: Option<i64>,
    pub transfers_count: Option<i64>,
    #[sea_orm(from_expr = r#"token_chain_infos_expr()"#)]
    pub chain_info: ChainInfo,
}

#[derive(Debug, Clone, FromJsonQueryResult, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub holders_count: Option<BigDecimal>,
    pub total_supply: Option<BigDecimal>,
    pub is_verified_contract: bool,
    pub contract_name: Option<String>,
}

impl From<ChainInfo> for proto::aggregated_token_info::ChainInfo {
    fn from(value: ChainInfo) -> Self {
        Self {
            holders_count: value.holders_count.map(|h| h.to_string()),
            total_supply: value.total_supply.map(|t| t.to_plain_string()),
            is_verified: value.is_verified_contract,
            contract_name: value.contract_name,
        }
    }
}

impl From<AggregatedToken> for proto::AggregatedTokenInfo {
    fn from(value: AggregatedToken) -> Self {
        Self {
            address_hash: value.address_hash.to_string(),
            circulating_market_cap: value.circulating_market_cap.map(|c| c.to_string()),
            decimals: value.decimals.map(|d| d.to_string()),
            icon_url: value.icon_url,
            name: value.name,
            symbol: value.symbol,
            r#type: db_token_type_to_proto_token_type(value.token_type).into(),
            exchange_rate: value.fiat_value.map(|f| f.to_string()),
            holders_count: value.holders_count.map(|h| h.to_string()),
            total_supply: value.total_supply.map(|t| t.to_plain_string()),
            chain_infos: BTreeMap::from([(
                value.chain_info.chain_id.to_string(),
                value.chain_info.into(),
            )]),
        }
    }
}

impl From<AggregatedToken> for proto::Token {
    fn from(v: AggregatedToken) -> Self {
        Self {
            address: v.address_hash.to_string(),
            name: v.name,
            symbol: v.symbol,
            icon_url: v.icon_url,
            chain_id: v.chain_info.chain_id.to_string(),
            is_verified_contract: v.chain_info.is_verified_contract,
        }
    }
}

impl From<AggregatedToken> for proto::Address {
    fn from(v: AggregatedToken) -> Self {
        Self {
            hash: v.address_hash.to_string(),
            contract_name: v.chain_info.contract_name,
            token_name: v.name,
            token_type: db_token_type_to_proto_token_type(v.token_type).into(),
            is_contract: Some(true),
            is_verified_contract: Some(v.chain_info.is_verified_contract),
            is_token: Some(true),
            chain_id: v.chain_info.chain_id.to_string(),
            domain_info: None,
        }
    }
}

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct TokenListUpdate {
    pub address_hash: SeaOrmAddress,
    pub chain_id: ChainId,
    pub updated_at: DateTime,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub token_type: TokenType,
}

impl From<TokenListUpdate> for proto::list_token_updates_response::TokenUpdate {
    fn from(v: TokenListUpdate) -> Self {
        Self {
            address_hash: v.address_hash.to_string(),
            chain_id: v.chain_id.to_string(),
            updated_at: v.updated_at.and_utc().timestamp_micros(),
            name: v.name,
            symbol: v.symbol,
            token_type: db_token_type_to_proto_token_type(v.token_type).into(),
        }
    }
}
