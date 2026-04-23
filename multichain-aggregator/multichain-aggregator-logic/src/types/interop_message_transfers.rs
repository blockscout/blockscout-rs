use crate::{
    error::ParseError,
    proto,
    types::{proto_address_hash_from_alloy, sea_orm_wrappers::SeaOrmAddress},
};
use alloy_primitives::Address;
use entity::interop_messages_transfers::{ActiveModel, Entity, Model};
use sea_orm::{ActiveValue::Set, DerivePartialModel, prelude::BigDecimal};

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct InteropMessageTransfer {
    pub token_address_hash: Option<SeaOrmAddress>,
    pub from_address_hash: SeaOrmAddress,
    pub to_address_hash: SeaOrmAddress,
    pub amount: BigDecimal,
}

impl TryFrom<Model> for InteropMessageTransfer {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            token_address_hash: v
                .token_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?
                .map(Into::into),
            from_address_hash: Address::try_from(v.from_address_hash.as_slice())?.into(),
            to_address_hash: Address::try_from(v.to_address_hash.as_slice())?.into(),
            amount: v.amount,
        })
    }
}

impl From<InteropMessageTransfer> for ActiveModel {
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
