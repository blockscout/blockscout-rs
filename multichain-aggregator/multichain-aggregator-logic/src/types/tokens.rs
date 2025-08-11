use super::ChainId;
use crate::{error::ParseError, proto, types::addresses::db_token_type_to_proto_token_type};
use entity::tokens::{ActiveModel, Entity};
use sea_orm::{
    ActiveValue::{NotSet, Set, Unchanged},
    DeriveIntoActiveModel, DerivePartialModel, IntoActiveModel, IntoActiveValue,
    prelude::{BigDecimal, Decimal},
};

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

#[derive(DerivePartialModel, Clone, Debug)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct Token {
    pub address_hash: Vec<u8>,
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
}

impl TryFrom<Token> for proto::TokenInfo {
    type Error = ParseError;

    fn try_from(value: Token) -> Result<Self, Self::Error> {
        Ok(Self {
            address_hash: alloy_primitives::Address::try_from(value.address_hash.as_slice())?
                .to_string(),
            circulating_market_cap: value.circulating_market_cap.map(|c| c.to_string()),
            decimals: value.decimals.map(|d| d.to_string()),
            holders_count: value.holders_count.map(|h| h.to_string()),
            icon_url: value.icon_url,
            name: value.name,
            symbol: value.symbol,
            total_supply: value.total_supply.map(|t| t.to_plain_string()),
            r#type: db_token_type_to_proto_token_type(value.token_type).into(),
            exchange_rate: value.fiat_value.map(|f| f.to_string()),
        })
    }
}
