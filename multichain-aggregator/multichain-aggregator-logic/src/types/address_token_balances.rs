use super::ChainId;
use crate::{error::ParseError, proto, types};
use entity::{
    address_token_balances::{ActiveModel, Column, Entity, Model},
    tokens,
};
use sea_orm::{
    ActiveValue::Set,
    DerivePartialModel, IntoSimpleExpr,
    prelude::{BigDecimal, Expr},
    sea_query::SimpleExpr,
};

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

#[derive(DerivePartialModel, Clone, Debug)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct ExtendedAddressTokenBalance {
    pub id: i64,
    pub address_hash: Vec<u8>,
    pub value: BigDecimal,
    pub token_id: Option<BigDecimal>,
    #[sea_orm(nested)]
    pub token: types::tokens::Token,
    #[sea_orm(from_expr = r#"fiat_balance_query()"#)]
    pub fiat_balance: Option<BigDecimal>,
}

impl TryFrom<ExtendedAddressTokenBalance>
    for proto::list_address_tokens_response::TokenBalanceInfo
{
    type Error = ParseError;

    fn try_from(v: ExtendedAddressTokenBalance) -> Result<Self, Self::Error> {
        Ok(Self {
            token: Some(v.token.try_into()?),
            token_id: v.token_id.map(|t| t.to_string()),
            value: v.value.to_plain_string(),
        })
    }
}
