use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{interop_message_transfers::InteropMessageTransfer, proto_address_hash_from_alloy},
};
use alloy_primitives::{Address, Bytes, TxHash};
use chrono::{Duration, Utc};
use entity::{
    interop_messages::{ActiveModel, Model},
    interop_messages_transfers,
};
use sea_orm::{ActiveValue::Set, prelude::DateTime};
use std::str::FromStr;

const SEVEN_DAYS: Duration = Duration::days(7);

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
    pub transfer: Option<InteropMessageTransfer>,
}

#[derive(Debug, PartialEq)]
pub enum Status {
    Pending,
    Failed,
    Success,
    Expired,
}

impl From<Status> for proto::interop_message::Status {
    fn from(v: Status) -> Self {
        match v {
            Status::Pending => proto::interop_message::Status::Pending,
            Status::Failed => proto::interop_message::Status::Failed,
            Status::Success => proto::interop_message::Status::Success,
            Status::Expired => proto::interop_message::Status::Expired,
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
            transfer: None,
        }
    }

    pub fn status(&self) -> Status {
        match (self.relay_transaction_hash, self.failed) {
            (None, _) => {
                let now = Utc::now().naive_utc();
                let is_expired = self
                    .timestamp
                    .map(|t| t + SEVEN_DAYS < now)
                    .unwrap_or(false);

                if is_expired {
                    Status::Expired
                } else {
                    Status::Pending
                }
            }
            (_, Some(true)) => Status::Failed,
            _ => Status::Success,
        }
    }

    pub fn method(&self) -> Method {
        match self.transfer {
            Some(InteropMessageTransfer {
                token_address_hash: Some(_),
                ..
            }) => Method::SendERC20,
            Some(InteropMessageTransfer {
                token_address_hash: None,
                ..
            }) => Method::SendETH,
            _ => Method::SendMessage,
        }
    }

    pub fn message_type(&self) -> MessageType {
        self.method().into()
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

impl TryFrom<(Model, Option<interop_messages_transfers::Model>)> for InteropMessage {
    type Error = ParseError;

    fn try_from(
        (v, transfer): (Model, Option<interop_messages_transfers::Model>),
    ) -> Result<Self, Self::Error> {
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
            transfer: transfer.map(InteropMessageTransfer::try_from).transpose()?,
        })
    }
}

impl From<InteropMessage> for proto::InteropMessage {
    fn from(v: InteropMessage) -> Self {
        let status = proto::interop_message::Status::from(v.status()).into();
        let message_type = v.message_type().into();
        let method = v.method().into();
        Self {
            sender: v
                .sender_address_hash
                .map(|a| proto_address_hash_from_alloy(&a)),
            target: v
                .target_address_hash
                .map(|a| proto_address_hash_from_alloy(&a)),
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
            status,
            transfer: v
                .transfer
                .map(proto::interop_message::InteropMessageTransfer::from),
            message_type,
            method,
        }
    }
}

pub enum MessageDirection {
    From,
    To,
}

impl FromStr for MessageDirection {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "from" => Ok(MessageDirection::From),
            "to" => Ok(MessageDirection::To),
            _ => Err(ParseError::Custom(format!(
                "invalid message direction: {s}",
            ))),
        }
    }
}

pub enum Method {
    SendMessage,
    SendERC20,
    SendETH,
}

impl From<Method> for String {
    fn from(v: Method) -> Self {
        match v {
            Method::SendMessage => "sendMessage",
            Method::SendERC20 => "sendERC20",
            Method::SendETH => "sendETH",
        }
        .to_string()
    }
}

// Frontend-compatible message types as defined in
// https://github.com/blockscout/frontend/blob/main/ui/txs/TxType.tsx
pub enum MessageType {
    CoinTransfer,
    TokenTransfer,
    ContractCall,
}

impl From<Method> for MessageType {
    fn from(v: Method) -> Self {
        match v {
            Method::SendMessage => MessageType::ContractCall,
            Method::SendERC20 => MessageType::TokenTransfer,
            Method::SendETH => MessageType::CoinTransfer,
        }
    }
}

impl From<MessageType> for String {
    fn from(v: MessageType) -> Self {
        match v {
            MessageType::CoinTransfer => "coin_transfer",
            MessageType::TokenTransfer => "token_transfer",
            MessageType::ContractCall => "contract_call",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interop_message_status() {
        let mut message = InteropMessage::base(
            1,
            ChainId::from_str("1").unwrap(),
            ChainId::from_str("2").unwrap(),
        );

        message.timestamp = Some(Utc::now().naive_utc() - SEVEN_DAYS);
        assert_eq!(message.status(), Status::Expired);

        message.timestamp = Some(Utc::now().naive_utc());
        assert_eq!(message.status(), Status::Pending);

        message.relay_transaction_hash = Some(TxHash::repeat_byte(0));
        assert_eq!(message.status(), Status::Success);

        message.failed = Some(true);
        assert_eq!(message.status(), Status::Failed);
    }
}
