use super::{
    address_coin_balances::AddressCoinBalance,
    address_token_balances::AddressTokenBalance,
    addresses::{proto_token_type_to_db_token_type, Address},
    block_ranges::BlockRange,
    hashes::{proto_hash_type_to_db_hash_type, Hash},
    interop_message_transfers::InteropMessageTransfer,
    interop_messages::InteropMessage,
    ChainId,
};
use crate::{
    error::{ParseError, ServiceError},
    metrics::IMPORT_ENTITIES_COUNT,
    proto,
};
use chrono::NaiveDateTime;
use sea_orm::prelude::BigDecimal;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone)]
pub struct BatchImportRequest {
    pub block_ranges: Vec<BlockRange>,
    pub hashes: Vec<Hash>,
    pub addresses: Vec<Address>,
    pub interop_messages: Vec<(InteropMessage, Option<InteropMessageTransfer>)>,
    pub address_coin_balances: Vec<AddressCoinBalance>,
    pub address_token_balances: Vec<AddressTokenBalance>,
}

impl BatchImportRequest {
    pub fn record_metrics(&self) {
        macro_rules! calculate_entity_metrics {
            ($entities: expr, $get_chain_id: expr, $entity_name: expr) => {
                let mut entity_metrics = HashMap::new();
                for e in $entities {
                    *entity_metrics.entry($get_chain_id(e)).or_insert(0) += 1;
                }
                for (chain_id, count) in entity_metrics {
                    IMPORT_ENTITIES_COUNT
                        .with_label_values(&[chain_id.to_string().as_str(), $entity_name])
                        .inc_by(count);
                }
            };
        }

        calculate_entity_metrics!(
            &self.block_ranges,
            |br: &BlockRange| br.chain_id,
            "block_ranges"
        );
        calculate_entity_metrics!(&self.hashes, |h: &Hash| h.chain_id, "hashes");
        calculate_entity_metrics!(&self.addresses, |a: &Address| a.chain_id, "addresses");
        calculate_entity_metrics!(
            &self.interop_messages,
            |(m, _): &(InteropMessage, _)| {
                if m.init_transaction_hash.is_some() {
                    m.init_chain_id
                } else {
                    m.relay_chain_id
                }
            },
            "interop_messages"
        );
        calculate_entity_metrics!(
            &self.address_coin_balances,
            |b: &AddressCoinBalance| b.chain_id,
            "address_coin_balances"
        );
        calculate_entity_metrics!(
            &self.address_token_balances,
            |b: &AddressTokenBalance| b.chain_id,
            "address_token_balances"
        );
    }
}

impl TryFrom<proto::BatchImportRequest> for BatchImportRequest {
    type Error = ServiceError;

    fn try_from(value: proto::BatchImportRequest) -> Result<Self, Self::Error> {
        let chain_id = value.chain_id.parse().map_err(ParseError::from)?;
        Ok(Self {
            block_ranges: value
                .block_ranges
                .into_iter()
                .map(|br| (chain_id, br).into())
                .collect(),
            hashes: value
                .hashes
                .into_iter()
                .map(|h| (chain_id, h).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            addresses: value
                .addresses
                .into_iter()
                .map(|a| (chain_id, a).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            interop_messages: value
                .interop_messages
                .into_iter()
                .map(|m| InteropMessageWithTransfer::try_from((chain_id, m)).map(|a| (a.0, a.1)))
                .collect::<Result<Vec<_>, _>>()?,
            address_coin_balances: value
                .address_coin_balances
                .into_iter()
                .map(|cb| (chain_id, cb).try_into())
                .collect::<Result<Vec<_>, _>>()?,
            address_token_balances: value
                .address_token_balances
                .into_iter()
                .map(|atb| (chain_id, atb).try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

impl
    TryFrom<(
        ChainId,
        proto::batch_import_request::AddressCoinBalanceImport,
    )> for AddressCoinBalance
{
    type Error = ParseError;

    fn try_from(
        (chain_id, acb): (
            ChainId,
            proto::batch_import_request::AddressCoinBalanceImport,
        ),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id,
            address_hash: acb.address_hash.parse()?,
            value: acb.value.parse()?,
        })
    }
}

impl
    TryFrom<(
        ChainId,
        proto::batch_import_request::AddressTokenBalanceImport,
    )> for AddressTokenBalance
{
    type Error = ParseError;

    fn try_from(
        (chain_id, atb): (
            ChainId,
            proto::batch_import_request::AddressTokenBalanceImport,
        ),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id,
            address_hash: atb.address_hash.parse()?,
            token_address_hash: atb.token_address_hash.parse()?,
            value: atb.value.parse()?,
            token_id: atb.token_id.map(|s| s.parse()).transpose()?,
        })
    }
}

impl From<(ChainId, proto::batch_import_request::BlockRangeImport)> for BlockRange {
    fn from((chain_id, br): (ChainId, proto::batch_import_request::BlockRangeImport)) -> Self {
        Self {
            chain_id,
            min_block_number: br.min_block_number,
            max_block_number: br.max_block_number,
        }
    }
}

impl TryFrom<(ChainId, proto::batch_import_request::HashImport)> for Hash {
    type Error = ParseError;

    fn try_from(
        (chain_id, h): (ChainId, proto::batch_import_request::HashImport),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            chain_id,
            hash: h.hash.parse()?,
            hash_type: proto_hash_type_to_db_hash_type(h.hash_type()),
        })
    }
}

impl TryFrom<(ChainId, proto::batch_import_request::AddressImport)> for Address {
    type Error = ParseError;

    fn try_from(
        (chain_id, a): (ChainId, proto::batch_import_request::AddressImport),
    ) -> Result<Self, Self::Error> {
        let hash = a.hash.parse()?;
        let token_type = proto_token_type_to_db_token_type(a.token_type());

        Ok(Self {
            chain_id,
            hash,
            domain_info: None,
            contract_name: a.contract_name,
            token_name: a.token_name,
            token_type,
            is_contract: a.is_contract.unwrap_or(false),
            is_verified_contract: a.is_verified_contract.unwrap_or(false),
            is_token: a.is_token.unwrap_or(false),
        })
    }
}

struct InteropMessageWithTransfer(InteropMessage, Option<InteropMessageTransfer>);

impl TryFrom<(ChainId, proto::batch_import_request::InteropMessageImport)>
    for InteropMessageWithTransfer
{
    type Error = ParseError;

    fn try_from(
        (chain_id, m): (ChainId, proto::batch_import_request::InteropMessageImport),
    ) -> Result<Self, Self::Error> {
        let message = m.message.ok_or_else(|| {
            ParseError::Custom("interop message is missing or invalid".to_string())
        })?;
        let (msg, transfer) = match message {
            proto::batch_import_request::interop_message_import::Message::Init(init) => {
                let InteropMessageWithTransfer(msg, transfer) = init.try_into()?;
                if msg.init_chain_id != chain_id {
                    return Err(ParseError::ChainIdMismatch {
                        expected: chain_id,
                        actual: msg.init_chain_id,
                    });
                }
                (msg, transfer)
            }
            proto::batch_import_request::interop_message_import::Message::Relay(relay) => {
                let msg: InteropMessage = relay.try_into()?;
                if msg.relay_chain_id != chain_id {
                    return Err(ParseError::ChainIdMismatch {
                        expected: chain_id,
                        actual: msg.relay_chain_id,
                    });
                }
                (msg, None)
            }
        };

        Ok(Self(msg, transfer))
    }
}

impl TryFrom<proto::batch_import_request::interop_message_import::Init>
    for InteropMessageWithTransfer
{
    type Error = ParseError;

    fn try_from(
        m: proto::batch_import_request::interop_message_import::Init,
    ) -> Result<Self, Self::Error> {
        let sender_address_hash = Some(m.sender_address_hash.parse()?);
        let target_address_hash = Some(m.target_address_hash.parse()?);
        let init_transaction_hash = Some(m.init_transaction_hash.parse()?);
        let timestamp = Some(parse_timestamp_secs(m.timestamp)?);
        let payload = Some(m.payload.parse()?);

        let transfer = match m {
            proto::batch_import_request::interop_message_import::Init {
                transfer_token_address_hash,
                transfer_from_address_hash: Some(transfer_from_address_hash),
                transfer_to_address_hash: Some(transfer_to_address_hash),
                transfer_amount: Some(transfer_amount),
                ..
            } => {
                let token_address_hash =
                    transfer_token_address_hash.map(|s| s.parse()).transpose()?;
                let from_address_hash = transfer_from_address_hash.parse()?;
                let to_address_hash = transfer_to_address_hash.parse()?;
                let amount = BigDecimal::from_str(&transfer_amount)?;

                Some(InteropMessageTransfer {
                    token_address_hash,
                    from_address_hash,
                    to_address_hash,
                    amount,
                })
            }
            proto::batch_import_request::interop_message_import::Init {
                transfer_token_address_hash: None,
                transfer_from_address_hash: None,
                transfer_to_address_hash: None,
                transfer_amount: None,
                ..
            } => None,
            _ => {
                return Err(ParseError::Custom(
                    "invalid interop message: transfer fields are inconsistent".to_string(),
                ));
            }
        };

        let base_msg = {
            let nonce = m.nonce;
            let init_chain_id = m.init_chain_id.parse()?;
            let relay_chain_id = m.relay_chain_id.parse()?;

            InteropMessage::base(nonce, init_chain_id, relay_chain_id)
        };

        let msg = InteropMessage {
            sender_address_hash,
            target_address_hash,
            init_transaction_hash,
            timestamp,
            payload,
            ..base_msg
        };

        Ok(Self(msg, transfer))
    }
}

impl TryFrom<proto::batch_import_request::interop_message_import::Relay> for InteropMessage {
    type Error = ParseError;

    fn try_from(
        m: proto::batch_import_request::interop_message_import::Relay,
    ) -> Result<Self, Self::Error> {
        let base_msg = {
            let nonce = m.nonce;
            let init_chain_id = m.init_chain_id.parse()?;
            let relay_chain_id = m.relay_chain_id.parse()?;

            InteropMessage::base(nonce, init_chain_id, relay_chain_id)
        };

        let relay_transaction_hash = Some(m.relay_transaction_hash.parse()?);
        let failed = Some(m.failed);

        Ok(Self {
            relay_transaction_hash,
            failed,
            ..base_msg
        })
    }
}

fn parse_timestamp_secs(timestamp: i64) -> Result<NaiveDateTime, ParseError> {
    match chrono::DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => Ok(dt.naive_utc()),
        None => Err(ParseError::Custom(format!(
            "invalid timestamp: {timestamp}",
        ))),
    }
}
