use super::ChainId;
use crate::error::ParseError;
use entity::address_token_balances::{ActiveModel, Model};
use sea_orm::{prelude::Decimal, ActiveValue::Set};

#[derive(Debug, Clone)]
pub struct AddressTokenBalance {
    pub address_hash: alloy_primitives::Address,
    pub token_address_hash: alloy_primitives::Address,
    pub value: Decimal,
    pub chain_id: ChainId,
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
            ..Default::default()
        }
    }
}
