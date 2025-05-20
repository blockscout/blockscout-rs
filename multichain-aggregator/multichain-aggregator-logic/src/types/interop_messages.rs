use super::ChainId;
use crate::{error::ParseError, proto};
use alloy_primitives::{Address, Bytes, TxHash};
use entity::interop_messages::{ActiveModel, Model};
use sea_orm::{prelude::DateTime, ActiveValue::Set};

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
}

pub enum Status {
    Pending,
    Failed,
    Success,
}

impl From<Status> for proto::interop_message::Status {
    fn from(v: Status) -> Self {
        match v {
            Status::Pending => proto::interop_message::Status::Pending,
            Status::Failed => proto::interop_message::Status::Failed,
            Status::Success => proto::interop_message::Status::Success,
        }
    }
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
        }
    }

    pub fn status(&self) -> Status {
        match (self.relay_transaction_hash, self.failed) {
            (None, _) => Status::Pending,
            (_, Some(true)) => Status::Failed,
            _ => Status::Success,
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
        })
    }
}

impl From<InteropMessage> for proto::InteropMessage {
    fn from(v: InteropMessage) -> Self {
        let status = proto::interop_message::Status::from(v.status()).into();
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
            status,
        }
    }
}
