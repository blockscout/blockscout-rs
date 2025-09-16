use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{self, proto_address_hash_from_alloy, sea_orm_wrappers::SeaOrmAddress},
};
use entity::{
    address_token_balances::{ActiveModel, Column, Entity, Model},
    tokens,
};
use sea_orm::{
    ActiveValue::Set,
    DerivePartialModel, FromJsonQueryResult, IntoSimpleExpr,
    prelude::{BigDecimal, Expr},
    sea_query::SimpleExpr,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct AddressTokenBalance {
    pub address_hash: alloy_primitives::Address,
    pub token_address_hash: alloy_primitives::Address,
    pub value: Option<BigDecimal>,
    pub chain_id: ChainId,
    pub token_id: Option<BigDecimal>,
}

impl TryFrom<Model> for AddressTokenBalance {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            address_hash: alloy_primitives::Address::try_from(v.address_hash.as_slice())?,
            token_address_hash: alloy_primitives::Address::try_from(
                v.token_address_hash.as_slice(),
            )?,
            value: v.value,
            chain_id: v.chain_id,
            token_id: v.token_id,
        })
    }
}

impl From<AddressTokenBalance> for ActiveModel {
    fn from(v: AddressTokenBalance) -> Self {
        Self {
            address_hash: Set(v.address_hash.to_vec()),
            token_address_hash: Set(v.token_address_hash.to_vec()),
            value: Set(v.value),
            chain_id: Set(v.chain_id),
            token_id: Set(v.token_id),
            ..Default::default()
        }
    }
}

pub fn fiat_balance_query() -> SimpleExpr {
    Column::Value
        .into_simple_expr()
        .mul(tokens::Column::FiatValue.into_simple_expr())
        .div(Expr::cust_with_expr(
            "POWER(10,$1)::numeric",
            tokens::Column::Decimals.into_simple_expr(),
        ))
}

pub fn chain_values_expr() -> SimpleExpr {
    Expr::cust_with_exprs(
        "jsonb_build_object('chain_id',$1,'value',$2)",
        [
            Column::ChainId.into_simple_expr(),
            Column::Value.into_simple_expr(),
        ],
    )
}

#[derive(DerivePartialModel, Clone, Debug)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct TokenHolder {
    pub address_hash: SeaOrmAddress,
    pub token_id: Option<BigDecimal>,
    pub value: BigDecimal,
}

impl From<TokenHolder> for proto::list_token_holders_response::TokenHolder {
    fn from(v: TokenHolder) -> Self {
        Self {
            address: Some(proto_address_hash_from_alloy(&v.address_hash)),
            token_id: v.token_id.map(|t| t.to_plain_string()),
            value: v.value.to_plain_string(),
        }
    }
}

#[derive(DerivePartialModel, Clone, Debug)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct AggregatedAddressTokenBalance {
    pub id: i64,
    pub address_hash: Vec<u8>,
    pub chain_id: ChainId,
    pub value: BigDecimal,
    pub token_id: Option<BigDecimal>,
    #[sea_orm(nested)]
    pub token: types::tokens::AggregatedToken,
    #[sea_orm(from_expr = r#"fiat_balance_query()"#)]
    pub fiat_balance: Option<BigDecimal>,
    #[sea_orm(from_expr = r#"Expr::cust_with_expr("jsonb_build_array($1)", chain_values_expr())"#)]
    pub chain_values: Vec<ChainValue>,
}

#[derive(Debug, FromJsonQueryResult, Serialize, Deserialize, Clone)]
pub struct ChainValue {
    pub chain_id: ChainId,
    pub value: BigDecimal,
}

impl TryFrom<AggregatedAddressTokenBalance>
    for proto::list_address_tokens_response::AggregatedTokenBalanceInfo
{
    type Error = ParseError;

    fn try_from(v: AggregatedAddressTokenBalance) -> Result<Self, Self::Error> {
        Ok(Self {
            token: Some(v.token.try_into()?),
            token_id: v.token_id.map(|t| t.to_string()),
            value: v.value.to_plain_string(),
            chain_values: v
                .chain_values
                .into_iter()
                .map(|c| (c.chain_id.to_string(), c.value.to_plain_string()))
                .collect(),
        })
    }
}
