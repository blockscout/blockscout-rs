use alloy_primitives::Address;
use sea_orm::{prelude::BigDecimal, ActiveValue::Set};

#[derive(Debug, Clone)]
pub struct InteropMessageTransfer {
    pub token_address_hash: Option<Address>,
    pub from_address_hash: Address,
    pub to_address_hash: Address,
    pub amount: BigDecimal,
}

impl From<InteropMessageTransfer> for entity::interop_messages_transfers::ActiveModel {
    fn from(v: InteropMessageTransfer) -> Self {
        Self {
            token_address_hash: Set(v.token_address_hash.map(|a| a.to_vec())),
            from_address_hash: Set(v.from_address_hash.to_vec()),
            to_address_hash: Set(v.to_address_hash.to_vec()),
            amount: Set(v.amount),
            ..Default::default()
        }
    }
}
