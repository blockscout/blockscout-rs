use std::collections::HashMap;

use alloy::primitives::{Address, B256, U256};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::header::AmbHeader;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Direction {
    EthToGnosis,
    GnosisToEth,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AnnotatedEvent<T> {
    pub(crate) event: T,
    pub(crate) transaction_hash: B256,
    pub(crate) block_number: i64,
    pub(crate) block_timestamp: NaiveDateTime,
    pub(crate) source_chain_id: i64,
    pub(crate) destination_chain_id: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum SourceRequest {
    Affirmation(AnnotatedEvent<SourceRequestEvent>),
    Signature(AnnotatedEvent<SourceRequestEvent>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum DestinationExecution {
    Affirmation(AnnotatedEvent<DestinationExecutionEvent>),
    Relayed(AnnotatedEvent<DestinationExecutionEvent>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SourceRequestEvent {
    pub(crate) message_id: B256,
    pub(crate) encoded_data: Vec<u8>,
    pub(crate) application_calldata: Vec<u8>,
    pub(crate) header: AmbHeaderData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AmbHeaderData {
    pub(crate) message_id: B256,
    pub(crate) sender: Address,
    pub(crate) executor: Address,
    pub(crate) source_chain_id: i64,
    pub(crate) destination_chain_id: i64,
    pub(crate) payload_offset: usize,
}

impl From<AmbHeader> for AmbHeaderData {
    fn from(header: AmbHeader) -> Self {
        Self {
            message_id: header.message_id,
            sender: header.sender,
            executor: header.executor,
            source_chain_id: header.source_chain_id,
            destination_chain_id: header.destination_chain_id,
            payload_offset: header.payload_offset,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DestinationExecutionEvent {
    pub(crate) sender: Address,
    pub(crate) executor: Address,
    pub(crate) message_id: B256,
    pub(crate) status: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CollectedSignaturesEvent {
    pub(crate) authority_responsible_for_relay: Address,
    pub(crate) message_hash: B256,
    pub(crate) count: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ValidatorConfirmation {
    pub(crate) validator_address: Address,
    pub(crate) tx_hash: B256,
    pub(crate) block_number: u64,
    pub(crate) block_timestamp: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum DecodedPayload {
    OmnibridgeTransfer {
        token_src_address: Address,
        token_dst_address: Option<Address>,
        src_amount: U256,
        dst_amount: U256,
        sender: Address,
        recipient: Address,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct SourceTransferDetails {
    pub(crate) token: Address,
    pub(crate) sender: Address,
    pub(crate) amount: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct DestinationTransferDetails {
    pub(crate) token: Address,
    pub(crate) recipient: Address,
    pub(crate) amount: U256,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) direction: Option<Direction>,
    pub(crate) source_request: Option<SourceRequest>,
    pub(crate) signatures_collected: Option<AnnotatedEvent<CollectedSignaturesEvent>>,
    pub(crate) validator_confirmations: HashMap<Address, ValidatorConfirmation>,
    pub(crate) destination_execution: Option<DestinationExecution>,
    pub(crate) decoded_payload: Option<DecodedPayload>,
    pub(crate) source_transfer: Option<SourceTransferDetails>,
    pub(crate) destination_transfer: Option<DestinationTransferDetails>,
}

impl SourceRequest {
    pub(crate) fn event(&self) -> &AnnotatedEvent<SourceRequestEvent> {
        match self {
            Self::Affirmation(event) | Self::Signature(event) => event,
        }
    }
}

impl DestinationExecution {
    pub(crate) fn event(&self) -> &AnnotatedEvent<DestinationExecutionEvent> {
        match self {
            Self::Affirmation(event) | Self::Relayed(event) => event,
        }
    }
}
