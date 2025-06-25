use crate::{error::ParseError, proto, types::proto_address_hash_from_alloy};
use alloy_primitives::Address;
use sea_orm::{prelude::BigDecimal, ActiveValue::Set};

#[derive(Debug, Clone)]
pub struct InteropMessageTransfer {
    pub token_address_hash: Option<Address>,
    pub from_address_hash: Address,
    pub to_address_hash: Address,
    pub amount: BigDecimal,
}

impl TryFrom<entity::interop_messages_transfers::Model> for InteropMessageTransfer {
    type Error = ParseError;

    fn try_from(v: entity::interop_messages_transfers::Model) -> Result<Self, Self::Error> {
        Ok(Self {
            token_address_hash: v
                .token_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            from_address_hash: Address::try_from(v.from_address_hash.as_slice())?,
            to_address_hash: Address::try_from(v.to_address_hash.as_slice())?,
            amount: v.amount,
        })
    }
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

impl From<InteropMessageTransfer> for proto::interop_message::InteropMessageTransfer {
    fn from(v: InteropMessageTransfer) -> Self {
        Self {
            token: v
                .token_address_hash
                .map(|a| proto::interop_message::TokenDetails {
                    address_hash: a.to_checksum(None),
                }),
            from: Some(proto_address_hash_from_alloy(&v.from_address_hash)),
            to: Some(proto_address_hash_from_alloy(&v.to_address_hash)),
            total: Some(proto::interop_message::TransferTotal {
                value: v.amount.to_plain_string(),
            }),
        }
    }
}
