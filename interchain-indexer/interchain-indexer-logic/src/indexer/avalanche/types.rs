// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::primitives::{Address, B256, FixedBytes};
use serde::{Deserialize, Serialize};

use super::abi::{ITeleporterMessenger, ITokenHome, ITokenTransferrer};
use crate::protocol_metadata::UnresolvedDestination;

pub type MessageId = FixedBytes<32>;

/// Source-side ICTT event with contract address.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AnnotatedICTTSource<T> {
    pub(crate) event: T,
    /// The ICTT contract address that emitted this event (token source address)
    pub(crate) contract_address: Address,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum SentOrRouted {
    Sent(AnnotatedICTTSource<ITokenTransferrer::TokensSent>),
    Routed(AnnotatedICTTSource<ITokenHome::TokensRouted>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum SentOrRoutedAndCalled {
    Sent(AnnotatedICTTSource<ITokenTransferrer::TokensAndCallSent>),
    Routed(AnnotatedICTTSource<ITokenHome::TokensAndCallRouted>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum CallOutcome {
    Succeeded(ITokenTransferrer::CallSucceeded),
    Failed(ITokenTransferrer::CallFailed),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum TokenTransfer {
    Sent(
        Option<SentOrRouted>,
        Option<ITokenTransferrer::TokensWithdrawn>,
    ),
    SentAndCalled(Option<SentOrRoutedAndCalled>, Option<CallOutcome>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum MessageExecutionOutcome {
    /// Message execution succeeded - this is final for ICM.
    Succeeded(AnnotatedEvent<ITeleporterMessenger::MessageExecuted>),
    /// Message execution failed - can be retried via retryMessageExecution().
    Failed(Box<AnnotatedEvent<ITeleporterMessenger::MessageExecutionFailed>>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AnnotatedEvent<T> {
    pub(crate) event: T,
    pub(crate) transaction_hash: B256,
    pub(crate) block_number: i64,
    pub(crate) block_timestamp: chrono::NaiveDateTime,
    pub(crate) source_chain_id: i64,
    /// `None` only for a send event whose destination could not be
    /// resolved. receive/execution always know it: it is the local chain.
    pub(crate) destination_chain_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub(crate) struct Message {
    /// Source-side: SendCrossChainMessage event (required for message to be "ready").
    pub(crate) send: Option<AnnotatedEvent<ITeleporterMessenger::SendCrossChainMessage>>,
    /// Destination-side: ReceiveCrossChainMessage event.
    pub(crate) receive: Option<AnnotatedEvent<ITeleporterMessenger::ReceiveCrossChainMessage>>,
    /// Execution outcome - may come in same transaction as receive, or later via retryMessageExecution().
    pub(crate) execution: Option<MessageExecutionOutcome>,
    /// ICTT token transfer (optional, only for ICTT messages).
    pub(crate) transfer: Option<TokenTransfer>,
    /// True when source chain is not in chain_ids (unknown chain scenario).
    /// When true, consolidation can proceed without the send event,
    /// using destination-side timestamps as `init_timestamp`.
    pub(crate) source_chain_is_unknown: bool,
    /// Diagnostics for an unresolved destination. Lives as long as the
    /// numeric destination is unknown; `consolidate()` decides whether to
    /// write it.
    #[serde(default)]
    pub(crate) unresolved_destination: Option<UnresolvedDestination>,
}
