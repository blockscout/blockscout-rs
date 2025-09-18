use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{addresses::db_token_type_to_proto_token_type, sea_orm_wrappers::SeaOrmAddress},
};
use entity::tokens::{ActiveModel, Column, Entity};
use sea_orm::{
    ActiveValue::{NotSet, Set, Unchanged},
    DeriveIntoActiveModel, DerivePartialModel, FromJsonQueryResult, IntoActiveModel,
    IntoActiveValue, IntoSimpleExpr,
    prelude::{BigDecimal, ColumnTrait, Decimal, Expr},
    sea_query::SimpleExpr,
};
use serde::{Deserialize, Serialize};

pub type TokenType = entity::sea_orm_active_enums::TokenType;

#[derive(Debug, Clone, Default)]
pub struct TokenUpdate {
    pub metadata: Option<UpdateTokenMetadata>,
    pub price_data: Option<UpdateTokenPriceData>,
    pub counters: Option<UpdateTokenCounters>,
    pub r#type: Option<UpdateTokenType>,
}

#[derive(Debug, Clone)]
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
        "jsonb_build_object('chain_id',$1,'holders_count',$2,'total_supply',$3)",
        [
            Column::ChainId.into_simple_expr(),
            Column::HoldersCount.into_simple_expr(),
            Column::TotalSupply.into_simple_expr(),
        ],
    )
}

#[derive(Debug, Clone, DerivePartialModel, Serialize, Deserialize)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct AggregatedToken {
    pub address_hash: SeaOrmAddress,
    pub chain_id: ChainId,
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
    #[sea_orm(
        from_expr = r#"Expr::cust_with_expr("jsonb_build_array($1)", token_chain_infos_expr())"#
    )]
    pub chain_infos: Vec<ChainInfo>,
    #[sea_orm(from_expr = r#"entity::addresses::Column::IsVerifiedContract.if_null(false)"#)]
    pub is_verified_contract: bool,
}

#[derive(Debug, Clone, FromJsonQueryResult, Serialize, Deserialize)]
pub struct ChainInfo {
    pub chain_id: ChainId,
    pub holders_count: Option<BigDecimal>,
    pub total_supply: Option<BigDecimal>,
}

impl From<ChainInfo> for proto::aggregated_token_info::ChainInfo {
    fn from(value: ChainInfo) -> Self {
        Self {
            holders_count: value.holders_count.map(|h| h.to_string()),
            total_supply: value.total_supply.map(|t| t.to_plain_string()),
        }
    }
}

impl TryFrom<AggregatedToken> for proto::AggregatedTokenInfo {
    type Error = ParseError;

    fn try_from(value: AggregatedToken) -> Result<Self, Self::Error> {
        Ok(Self {
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
            chain_infos: value
                .chain_infos
                .into_iter()
                .map(|c| (c.chain_id.to_string(), c.into()))
                .collect(),
        })
    }
}

impl From<AggregatedToken> for proto::Token {
    fn from(v: AggregatedToken) -> Self {
        Self {
            address: v.address_hash.to_string(),
            name: v.name,
            symbol: v.symbol,
            icon_url: v.icon_url,
            chain_id: v.chain_id.to_string(),
            is_verified_contract: v.is_verified_contract,
        }
    }
}
