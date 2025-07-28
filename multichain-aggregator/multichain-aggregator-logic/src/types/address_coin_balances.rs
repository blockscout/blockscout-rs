use super::ChainId;
use crate::error::ParseError;
use entity::address_coin_balances::{ActiveModel, Model};
use sea_orm::{ActiveValue::Set, prelude::BigDecimal};

#[derive(Debug, Clone)]
pub struct AddressCoinBalance {
    pub address_hash: alloy_primitives::Address,
    pub value: BigDecimal,
    pub chain_id: ChainId,
}

impl TryFrom<Model> for AddressCoinBalance {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            address_hash: alloy_primitives::Address::try_from(v.address_hash.as_slice())?,
            value: v.value,
            chain_id: v.chain_id,
        })
    }
}

impl From<AddressCoinBalance> for ActiveModel {
    fn from(v: AddressCoinBalance) -> Self {
        Self {
            address_hash: Set(v.address_hash.to_vec()),
            value: Set(v.value),
            chain_id: Set(v.chain_id),
            ..Default::default()
        }
    }
}
