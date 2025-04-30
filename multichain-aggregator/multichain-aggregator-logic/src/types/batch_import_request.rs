use super::{
    addresses::{proto_token_type_to_db_token_type, Address},
    block_ranges::BlockRange,
    hashes::{proto_hash_type_to_db_hash_type, Hash},
    interop_messages::InteropMessage,
    ChainId,
};
use crate::{
    error::{ParseError, ServiceError},
    metrics::IMPORT_ENTITIES_COUNT,
    proto,
};
use std::collections::HashMap;
use chrono::NaiveDateTime;

#[derive(Debug, Clone)]
pub struct BatchImportRequest {
    pub block_ranges: Vec<BlockRange>,
    pub hashes: Vec<Hash>,
    pub addresses: Vec<Address>,
    pub interop_messages: Vec<InteropMessage>,
}

impl BatchImportRequest {
    pub fn record_metrics(&self) {
        macro_rules! calculate_entity_metrics {
            ($entities: expr, $entity_name: expr) => {
                let mut entity_metrics = HashMap::new();
                for e in $entities {
                    *entity_metrics.entry(e.chain_id).or_insert(0) += 1;
                }
                for (chain_id, count) in entity_metrics {
                    IMPORT_ENTITIES_COUNT
                        .with_label_values(&[chain_id.to_string().as_str(), $entity_name])
                        .inc_by(count);
                }
            };
        }

        calculate_entity_metrics!(&self.block_ranges, "block_ranges");
        calculate_entity_metrics!(&self.hashes, "hashes");
        calculate_entity_metrics!(&self.addresses, "addresses");
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
                .map(|m| (chain_id, m).try_into())
                .collect::<Result<Vec<_>, _>>()?,
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

impl TryFrom<(ChainId, proto::batch_import_request::InteropMessageImport)> for InteropMessage {
    type Error = ParseError;

    fn try_from(
        (chain_id, m): (ChainId, proto::batch_import_request::InteropMessageImport),
    ) -> Result<Self, Self::Error> {
        let sender = m.sender.map(|s| s.parse()).transpose()?;
        let target = m.target.map(|s| s.parse()).transpose()?;
        let nonce = m.nonce;
        let init_chain_id = m.init_chain_id.parse()?;
        let init_transaction_hash = m.init_transaction_hash.map(|s| s.parse()).transpose()?;
        let block_number = m.block_number;
        let timestamp = m.timestamp.map(parse_timestamp_secs).transpose()?;
        let relay_chain_id = m.relay_chain_id.parse()?;
        let relay_transaction_hash = m.relay_transaction_hash.map(|s| s.parse()).transpose()?;
        let payload = m.payload.map(|s| s.parse()).transpose()?;
        let failed = m.failed;

        if init_chain_id != chain_id && relay_chain_id != chain_id {
            let err = ParseError::Custom("interop message chain id mismatch".to_string());
            tracing::error!(
                init_chain_id = ?init_chain_id,
                relay_chain_id = ?relay_chain_id,
                err = ?err,
            );
            return Err(err);
        }

        let message = Self {
            sender,
            target,
            nonce,
            init_chain_id,
            init_transaction_hash,
            block_number,
            timestamp,
            relay_chain_id,
            relay_transaction_hash,
            payload,
            failed,
        };

        // TODO: support partial `init` and `relay` messages
        // but with consistency checks
        let is_full_message = sender.is_some()
            && target.is_some()
            && init_transaction_hash.is_some()
            && block_number.is_some()
            && timestamp.is_some()
            && relay_transaction_hash.is_some()
            && message.payload.is_some()
            && failed.is_some();

        if !is_full_message {
            let err = ParseError::Custom("inconsistent interop message".to_string());
            tracing::error!(
                message = ?message,
                err = ?err,
            );
            return Err(err);
        }

        Ok(message)
    }
}

fn parse_timestamp_secs(timestamp: i64) -> Result<NaiveDateTime, ParseError> {
    match chrono::DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => Ok(dt.naive_utc()),
        None => Err(ParseError::Custom(format!(
            "invalid timestamp: {}",
            timestamp
        ))),
    }
}
