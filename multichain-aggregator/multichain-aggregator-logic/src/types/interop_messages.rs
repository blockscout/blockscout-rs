use super::ChainId;
use crate::{error::ParseError, proto};
use alloy_primitives::{Address, Bytes, TxHash};
use entity::interop_messages::{ActiveModel, Model};
use sea_orm::{prelude::DateTime, ActiveValue::Set};

#[derive(Debug, Clone)]
pub struct InteropMessage {
    pub sender: Option<Address>,
    pub target: Option<Address>,
    pub nonce: i64,
    pub init_chain_id: ChainId,
    pub init_transaction_hash: Option<TxHash>,
    pub block_number: Option<u64>,
    pub timestamp: Option<DateTime>,
    pub relay_chain_id: ChainId,
    pub relay_transaction_hash: Option<TxHash>,
    pub payload: Option<Bytes>,
    pub failed: Option<bool>,
}

impl From<InteropMessage> for ActiveModel {
    fn from(v: InteropMessage) -> Self {
        Self {
            sender: Set(v.sender.map(|a| a.to_vec())),
            target: Set(v.target.map(|a| a.to_vec())),
            nonce: Set(v.nonce),
            init_chain_id: Set(v.init_chain_id),
            init_transaction_hash: Set(v.init_transaction_hash.map(|h| h.to_vec())),
            block_number: Set(v.block_number.map(|n| n as i64)),
            timestamp: Set(v.timestamp),
            relay_chain_id: Set(v.relay_chain_id),
            relay_transaction_hash: Set(v.relay_transaction_hash.map(|h| h.to_vec())),
            payload: Set(v.payload.map(|p| p.to_vec())),
            failed: Set(v.failed),
            ..Default::default()
        }
    }
}

impl TryFrom<Model> for InteropMessage {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            sender: v
                .sender
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            target: v
                .target
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?,
            nonce: v.nonce,
            init_chain_id: v.init_chain_id,
            init_transaction_hash: v
                .init_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?,
            block_number: v.block_number.map(|n| n as u64),
            timestamp: v.timestamp,
            relay_chain_id: v.relay_chain_id,
            relay_transaction_hash: v
                .relay_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?,
            payload: v.payload.map(Bytes::from),
            failed: v.failed,
        })
    }
}

impl From<InteropMessage> for proto::InteropMessage {
    fn from(v: InteropMessage) -> Self {
        Self {
            sender: v.sender.map(|a| a.to_string()),
            target: v.target.map(|a| a.to_string()),
            nonce: v.nonce,
            init_chain_id: v.init_chain_id.to_string(),
            init_transaction_hash: v.init_transaction_hash.map(|h| h.to_string()),
            block_number: v.block_number,
            timestamp: v.timestamp.map(|t| {
                t.and_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            }),
            relay_chain_id: v.relay_chain_id.to_string(),
            relay_transaction_hash: v.relay_transaction_hash.map(|h| h.to_string()),
            payload: v.payload.map(|p| p.to_string()),
            failed: v.failed,
        }
    }
}
