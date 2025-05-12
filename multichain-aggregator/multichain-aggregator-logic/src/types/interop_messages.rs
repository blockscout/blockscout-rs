use super::ChainId;
use crate::{error::ParseError, proto};
use alloy_primitives::{Address, Bytes, TxHash};
use entity::interop_messages::{ActiveModel, Model};
use sea_orm::{
    prelude::{DateTime, Decimal},
    ActiveValue::Set,
};

#[derive(Debug, Clone)]
pub struct InteropMessage {
    pub sender_address_hash: Option<Address>,
    pub target_address_hash: Option<Address>,
    pub nonce: i64,
    pub init_chain_id: ChainId,
    pub init_transaction_hash: Option<TxHash>,
    pub timestamp: Option<DateTime>,
    pub relay_chain_id: ChainId,
    pub relay_transaction_hash: Option<TxHash>,
    pub payload: Option<Bytes>,
    pub failed: Option<bool>,
    pub transfer_token_address_hash: Option<Address>,
    pub transfer_from_address_hash: Option<Address>,
    pub transfer_to_address_hash: Option<Address>,
    pub transfer_amount: Option<Decimal>,
}

impl InteropMessage {
    pub fn base(nonce: i64, init_chain_id: ChainId, relay_chain_id: ChainId) -> Self {
        Self {
            nonce,
            init_chain_id,
            relay_chain_id,

            sender_address_hash: None,
            target_address_hash: None,
            init_transaction_hash: None,
            timestamp: None,
            relay_transaction_hash: None,
            payload: None,
            failed: None,
            transfer_token_address_hash: None,
            transfer_from_address_hash: None,
            transfer_to_address_hash: None,
            transfer_amount: None,
        }
    }
}

impl From<InteropMessage> for ActiveModel {
    fn from(v: InteropMessage) -> Self {
        Self {
            sender_address_hash: Set(v.sender_address_hash.map(|a| a.to_vec())),
            target_address_hash: Set(v.target_address_hash.map(|a| a.to_vec())),
            nonce: Set(v.nonce),
            init_chain_id: Set(v.init_chain_id),
            init_transaction_hash: Set(v.init_transaction_hash.map(|h| h.to_vec())),
            timestamp: Set(v.timestamp),
            relay_chain_id: Set(v.relay_chain_id),
            relay_transaction_hash: Set(v.relay_transaction_hash.map(|h| h.to_vec())),
            payload: Set(v.payload.map(|p| p.to_vec())),
            failed: Set(v.failed),
            transfer_token_address_hash: Set(v.transfer_token_address_hash.map(|a| a.to_vec())),
            transfer_from_address_hash: Set(v.transfer_from_address_hash.map(|a| a.to_vec())),
            transfer_to_address_hash: Set(v.transfer_to_address_hash.map(|a| a.to_vec())),
            transfer_amount: Set(v.transfer_amount),
            ..Default::default()
        }
    }
}

impl TryFrom<Model> for InteropMessage {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_address_hash: v
                .sender_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            target_address_hash: v
                .target_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            nonce: v.nonce,
            init_chain_id: v.init_chain_id,
            init_transaction_hash: v
                .init_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?,
            timestamp: v.timestamp,
            relay_chain_id: v.relay_chain_id,
            relay_transaction_hash: v
                .relay_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?,
            payload: v.payload.map(Bytes::from),
            failed: v.failed,
            transfer_token_address_hash: v
                .transfer_token_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            transfer_from_address_hash: v
                .transfer_from_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            transfer_to_address_hash: v
                .transfer_to_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            transfer_amount: v.transfer_amount,
        })
    }
}

impl From<InteropMessage> for proto::InteropMessage {
    fn from(v: InteropMessage) -> Self {
        Self {
            sender_address_hash: v.sender_address_hash.map(|a| a.to_string()),
            target_address_hash: v.target_address_hash.map(|a| a.to_string()),
            nonce: v.nonce,
            init_chain_id: v.init_chain_id.to_string(),
            init_transaction_hash: v.init_transaction_hash.map(|h| h.to_string()),
            timestamp: v.timestamp.map(|t| {
                t.and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            }),
            relay_chain_id: v.relay_chain_id.to_string(),
            relay_transaction_hash: v.relay_transaction_hash.map(|h| h.to_string()),
            payload: v.payload.map(|p| p.to_string()),
            failed: v.failed,
            transfer_token_address_hash: v.transfer_token_address_hash.map(|a| a.to_string()),
            transfer_from_address_hash: v.transfer_from_address_hash.map(|a| a.to_string()),
            transfer_to_address_hash: v.transfer_to_address_hash.map(|a| a.to_string()),
            transfer_amount: v.transfer_amount.map(|a| a.to_string()),
        }
    }
}
