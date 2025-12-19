use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};

use super::abi::{ITeleporterMessenger, ITokenHome, ITokenTransferrer};

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
    pub(crate) chain_id: i64,
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
}
