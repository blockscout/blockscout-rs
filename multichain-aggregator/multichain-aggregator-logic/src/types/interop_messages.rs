use super::ChainId;
use crate::{
    error::ParseError,
    proto,
    types::{
        interop_message_transfers::InteropMessageTransfer,
        proto_address_hash_from_alloy,
        sea_orm_wrappers::{SeaOrmAddress, SeaOrmB256, SeaOrmBytes},
    },
};
use alloy_primitives::{Address, Bytes, TxHash};
use chrono::{Duration, Utc};
use entity::interop_messages::{ActiveModel, Entity, Model};
use sea_orm::{ActiveValue::Set, DerivePartialModel, prelude::DateTime};
use std::str::FromStr;

const SEVEN_DAYS: Duration = Duration::days(7);

#[derive(DerivePartialModel, Debug, Clone)]
#[sea_orm(entity = "Entity", from_query_result)]
pub struct ExtendedInteropMessage {
    pub sender_address_hash: Option<SeaOrmAddress>,
    pub target_address_hash: Option<SeaOrmAddress>,
    pub nonce: i64,
    pub init_chain_id: ChainId,
    pub init_transaction_hash: Option<SeaOrmB256>,
    pub timestamp: Option<DateTime>,
    pub relay_chain_id: ChainId,
    pub relay_transaction_hash: Option<SeaOrmB256>,
    pub payload: Option<SeaOrmBytes>,
    pub failed: Option<bool>,
    #[sea_orm(nested)]
    pub transfer: Option<InteropMessageTransfer>,
    #[sea_orm(skip)]
    pub decoded_payload: Option<serde_json::Value>,
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

impl ExtendedInteropMessage {
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
            decoded_payload: None,
        }
    }

    pub fn status(&self) -> Status {
        match (&self.relay_transaction_hash, self.failed) {
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

impl From<ExtendedInteropMessage> for ActiveModel {
    fn from(v: ExtendedInteropMessage) -> Self {
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

impl TryFrom<Model> for ExtendedInteropMessage {
    type Error = ParseError;

    fn try_from(v: Model) -> Result<Self, Self::Error> {
        Ok(Self {
            sender_address_hash: v
                .sender_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?
                .map(Into::into),
            target_address_hash: v
                .target_address_hash
                .map(|a| Address::try_from(a.as_slice()))
                .transpose()?
                .map(Into::into),
            nonce: v.nonce,
            init_chain_id: v.init_chain_id,
            init_transaction_hash: v
                .init_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?
                .map(Into::into),
            timestamp: v.timestamp,
            relay_chain_id: v.relay_chain_id,
            relay_transaction_hash: v
                .relay_transaction_hash
                .map(|h| TxHash::try_from(h.as_slice()))
                .transpose()?
                .map(Into::into),
            payload: v.payload.map(Bytes::from).map(Into::into),
            failed: v.failed,
            transfer: None,
            decoded_payload: None,
        })
    }
}

impl From<ExtendedInteropMessage> for proto::InteropMessage {
    fn from(v: ExtendedInteropMessage) -> Self {
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
            decoded_payload: v.decoded_payload.and_then(|p| {
                serde_json::from_value(p)
                    .inspect_err(|_| {
                        tracing::error!("failed to deserialize protocol");
                    })
                    .ok()
            }),
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
        let mut message = ExtendedInteropMessage::base(
            1,
            ChainId::from_str("1").unwrap(),
            ChainId::from_str("2").unwrap(),
        );

        message.timestamp = Some(Utc::now().naive_utc() - SEVEN_DAYS);
        assert_eq!(message.status(), Status::Expired);

        message.timestamp = Some(Utc::now().naive_utc());
        assert_eq!(message.status(), Status::Pending);

        message.relay_transaction_hash = Some(TxHash::repeat_byte(0).into());
        assert_eq!(message.status(), Status::Success);

        message.failed = Some(true);
        assert_eq!(message.status(), Status::Failed);
    }
}
