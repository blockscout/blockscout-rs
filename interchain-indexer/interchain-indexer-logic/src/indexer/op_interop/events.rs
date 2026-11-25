// SPDX-License-Identifier: LicenseRef-Blockscout

//! Per-transaction handling of OP interop logs, in two phases:
//!
//! 1. [`plan_transaction`] — pure: resolve the contract version, decode,
//!    collect every `ExecutingMessage` and each relay's candidate executions,
//!    apply the skip rules. It returns the actions to take, the executions and
//!    what it observed, and touches nothing. [`plan_and_report`] adds the
//!    metrics and logs of the observations.
//! 2. [`apply_actions`] — apply the actions to the buffer.
//!
//! Splitting them means a decode problem cannot leave a half-applied
//! transaction, and every decision is testable as a value.
//!
//! There is no state shared between transactions: no queue, no lookup map. A
//! replay of a block reconstructs everything the first attempt derived from
//! it.

use std::{collections::HashSet, sync::Arc};

use alloy::{
    primitives::{Address, B256, ChainId, U256},
    rpc::types::Log,
};
use anyhow::{Context, Result, anyhow, bail, ensure};

use crate::message_buffer::MessageBuffer;

use super::{
    abi::{ContractKind, LogResolution, OpInteropRegistry, v1},
    eth_transfer::{EthPayload, EthTransfer, classify},
    metrics,
    types::{
        DestinationLeg, ExecutingLeg, LegOutcome, LogRef, Message, RelayEth, SourceLeg,
        key_from_message_hash, message_hash, payload_hash,
    },
    version::{L2_TO_L2_MESSENGER, OpInteropEvent, grammar_for},
};

/// Something a transaction asks the buffer to do.
#[derive(Clone, Debug, PartialEq, Eq)]
// Short-lived values, a handful per transaction, consumed immediately: boxing
// the relay leg would only add an allocation.
#[allow(clippy::large_enum_variant)]
pub(super) enum TxAction {
    Send(SourceLeg),
    Relay(DestinationLeg),
}

/// Why a message was not indexed. `as_str` is the `reason` metric label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SkipReason {
    DestinationNotConfigured,
    DestinationOutOfRange,
    SourceNotConfigured,
    SourceOutOfRange,
}

impl SkipReason {
    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::DestinationNotConfigured => "destination_not_configured",
            Self::DestinationOutOfRange => "destination_out_of_range",
            Self::SourceNotConfigured => "source_not_configured",
            Self::SourceOutOfRange => "source_out_of_range",
        }
    }
}

/// What [`plan_transaction`] saw besides the actions: input for metrics and
/// logs. Returned instead of emitted so the planner stays pure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Observation {
    /// A log of ours was decoded.
    Event(OpInteropEvent),
    /// A message whose other endpoint is outside the bridge.
    Skipped(SkipReason),
    /// A decoded ExecutingMessage; written to the ledger as a raw execution.
    Execution(ExecutionOrigin),
    /// A relay with no candidate in its relay window. Still indexed.
    RelayUnpaired,
    /// A log whose topic belongs to another version window of its contract.
    WrongVersion,
    /// A bridge-to-bridge `SentMessage` whose payload is not an ETH transfer
    /// of the known grammar. The message is still indexed, without a transfer.
    EthPayloadUnrecognized,
    /// An ETH transfer message with no `SendETH` after it in its transaction.
    SendEthMissing,
    /// A `SendETH` with no ETH transfer message before it.
    SendEthUnpaired,
    /// A `SendETH` that disagrees with the ETH transfer message before it.
    SendEthMismatch,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct TxPlan {
    pub(super) actions: Vec<TxAction>,
    /// Skips and anomalies, for metrics and logs.
    pub(super) observations: Vec<Observation>,
    /// Every decoded ExecutingMessage of the transaction, of any origin,
    /// ascending by log index: one ledger row each.
    pub(super) executions: Vec<ExecutingLeg>,
}

/// Whose log an `ExecutingMessage` reads. `as_str` is the `origin` metric label.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExecutionOrigin {
    Messenger,
    Foreign,
}

impl ExecutionOrigin {
    pub(super) fn of(origin: Address) -> Self {
        if origin == L2_TO_L2_MESSENGER {
            Self::Messenger
        } else {
            Self::Foreign
        }
    }

    pub(super) fn as_str(&self) -> &'static str {
        match self {
            Self::Messenger => "messenger",
            Self::Foreign => "foreign",
        }
    }
}

/// Whether an on-chain `uint256` chain id names a chain of this bridge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChainRef {
    Configured(ChainId),
    NotConfigured,
    /// Does not fit the chain-id domain (`u64`) at all.
    OutOfRange,
}

/// Configured ids come from `i64` config fields, so membership implies the
/// value also fits the storage type.
pub(super) fn classify_chain(value: U256, configured: &HashSet<ChainId>) -> ChainRef {
    match ChainId::try_from(value) {
        Ok(chain_id) if configured.contains(&chain_id) => ChainRef::Configured(chain_id),
        Ok(_) => ChainRef::NotConfigured,
        Err(_) => ChainRef::OutOfRange,
    }
}

/// Everything this indexer keeps about where a log was seen, taken from the
/// `eth_getLogs` row. A row without one of these fields is not a skip: the
/// caller must fail the block, or the checkpoint would certify a block whose
/// data was not used.
pub(super) fn log_ref(chain_id: ChainId, log: &Log) -> Result<LogRef> {
    ensure!(!log.removed, "log is marked as removed");
    let block_number = log.block_number.context("log has no block number")?;
    let block_hash = log.block_hash.context("log has no block hash")?;
    let transaction_hash = log
        .transaction_hash
        .context("log has no transaction hash")?;
    let log_index = log.log_index.context("log has no log index")?;
    let timestamp = log
        .block_timestamp
        .context("log has no block timestamp (the RPC does not return blockTimestamp)")?;

    // The storage types are `i64`; checked here, once, so that building the
    // database models later cannot fail.
    ensure!(
        i64::try_from(block_number).is_ok(),
        "log block number {block_number} does not fit i64"
    );
    ensure!(
        i64::try_from(log_index).is_ok(),
        "log index {log_index} does not fit i64"
    );
    let block_timestamp = i64::try_from(timestamp)
        .ok()
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map(|datetime| datetime.naive_utc())
        .with_context(|| format!("log block timestamp {timestamp} is not a valid time"))?;

    Ok(LogRef {
        chain_id,
        block_number,
        block_hash,
        block_timestamp,
        transaction_hash,
        log_index,
    })
}

/// A decoded log of ours.
enum Decoded {
    Executing(ExecutingLeg),
    Sent {
        log: LogRef,
        event: v1::SentMessage,
        payload_hash: alloy::primitives::B256,
    },
    Relayed {
        log: LogRef,
        event: v1::RelayedMessage,
    },
    SendEth(v1::SendETH),
    RelayEth(v1::RelayETH),
}

fn decode(
    chain_id: ChainId,
    kind: ContractKind,
    event: OpInteropEvent,
    log: &Log,
) -> Result<Decoded> {
    // One arm per grammar revision; revision 1 is the only one.
    ensure!(
        kind.grammar_version == 1,
        "no decoder for OP interop grammar version {} of {}",
        kind.grammar_version,
        kind.contract.as_str()
    );
    let log_ref = log_ref(chain_id, log)?;

    Ok(match event {
        OpInteropEvent::ExecutingMessage => {
            let decoded = log
                .log_decode::<v1::ExecutingMessage>()
                .context("failed to decode ExecutingMessage")?
                .inner
                .data;
            Decoded::Executing(ExecutingLeg {
                log: log_ref,
                msg_hash: decoded.msgHash,
                origin: decoded.id.origin,
                source_chain_id: decoded.id.chainId,
                source_block_number: decoded.id.blockNumber,
                source_log_index: decoded.id.logIndex,
                source_timestamp: decoded.id.timestamp,
            })
        }
        OpInteropEvent::SentMessage => Decoded::Sent {
            log: log_ref,
            event: log
                .log_decode::<v1::SentMessage>()
                .context("failed to decode SentMessage")?
                .inner
                .data,
            payload_hash: payload_hash(&log.inner),
        },
        OpInteropEvent::RelayedMessage => Decoded::Relayed {
            log: log_ref,
            event: log
                .log_decode::<v1::RelayedMessage>()
                .context("failed to decode RelayedMessage")?
                .inner
                .data,
        },
        OpInteropEvent::SendEth => Decoded::SendEth(
            log.log_decode::<v1::SendETH>()
                .context("failed to decode SendETH")?
                .inner
                .data,
        ),
        OpInteropEvent::RelayEth => Decoded::RelayEth(
            log.log_decode::<v1::RelayETH>()
                .context("failed to decode RelayETH")?
                .inner
                .data,
        ),
    })
}

/// Phase 1. `logs` must be the logs of one transaction, ascending by
/// `log_index` (catch-up batches arrive descending; the caller sorts).
///
/// Every decoded `ExecutingMessage`, of any origin, goes to
/// `TxPlan::executions`. The messenger-origin ones also go into a window.
/// Each `RelayedMessage` takes the window entries of its source chain as
/// candidates and clears the window. Relays never nest, so everything in the
/// window since the previous relay is either that relay's own execution or a
/// pull read; the choice between them is made by content in `consolidate`.
///
/// The ETH bridge events are paired by position, with the same single-slot
/// scheme: a `SentMessage` carrying an ETH transfer payload fills the send
/// slot and the `SendETH` after it consumes it; `RelayETH` fills the relay
/// slot and the `RelayedMessage` after it consumes it. Both are diagnostics:
/// they only ever add observations and `DestinationLeg.relay_eth`.
///
/// A decode failure is an `Err` for the whole transaction, with no actions.
pub(super) fn plan_transaction(
    registry: &OpInteropRegistry,
    configured_chains: &HashSet<ChainId>,
    chain_id: ChainId,
    logs: &[&Log],
) -> Result<TxPlan> {
    let mut plan = TxPlan::default();
    let mut window: Vec<ExecutingLeg> = Vec::new();
    let mut pending_send_eth: Option<(EthTransfer, U256)> = None;
    let mut pending_relay_eth: Option<RelayEth> = None;

    for log in logs {
        let Some(topic0) = log.topic0() else {
            continue;
        };
        let block_number = log.block_number.context("log has no block number")?;
        let kind = match registry.resolve_log(chain_id, log.address(), topic0, block_number) {
            LogResolution::Matched(_, kind) => kind,
            LogResolution::NotConfigured => continue,
            LogResolution::WrongVersion => {
                plan.observations.push(Observation::WrongVersion);
                continue;
            }
        };
        let Some(event) = grammar_for(kind.contract, kind.grammar_version)?.event_for_topic(topic0)
        else {
            bail!(
                "OP interop registry matched topic {topic0} that grammar version {} of {} \
                 does not declare",
                kind.grammar_version,
                kind.contract.as_str()
            );
        };
        let decoded = decode(chain_id, kind, event, log).with_context(|| {
            format!(
                "failed to read OP interop {} log (log_index={:?})",
                event.as_str(),
                log.log_index
            )
        })?;
        plan.observations.push(Observation::Event(event));

        match decoded {
            Decoded::Executing(exec) => {
                let origin = ExecutionOrigin::of(exec.origin);
                plan.observations.push(Observation::Execution(origin));
                if origin == ExecutionOrigin::Messenger {
                    // A new relay group may start: a RelayETH never precedes
                    // one of its own group.
                    pending_relay_eth = None;
                    window.push(exec.clone());
                }
                plan.executions.push(exec);
            }
            Decoded::Sent {
                log,
                event,
                payload_hash,
            } => {
                match classify(event.sender, event.target, &event.message) {
                    EthPayload::NotBridgeMessage => {}
                    EthPayload::Unrecognized => {
                        plan.observations.push(Observation::EthPayloadUnrecognized);
                    }
                    EthPayload::Transfer(transfer) => {
                        if pending_send_eth.is_some() {
                            plan.observations.push(Observation::SendEthMissing);
                        }
                        pending_send_eth = Some((transfer, event.destination));
                    }
                }
                match classify_chain(event.destination, configured_chains) {
                    ChainRef::Configured(destination_chain_id) => {
                        plan.actions.push(TxAction::Send(SourceLeg {
                            log,
                            destination_chain_id,
                            nonce: event.messageNonce,
                            sender: event.sender,
                            target: event.target,
                            // `source` of the message hash is the chain being
                            // scanned: it is not in the event.
                            message_hash: message_hash(
                                event.destination,
                                U256::from(chain_id),
                                event.messageNonce,
                                event.sender,
                                event.target,
                                &event.message,
                            ),
                            message: event.message,
                            payload_hash,
                        }));
                    }
                    ChainRef::NotConfigured => plan
                        .observations
                        .push(Observation::Skipped(SkipReason::DestinationNotConfigured)),
                    ChainRef::OutOfRange => plan
                        .observations
                        .push(Observation::Skipped(SkipReason::DestinationOutOfRange)),
                }
            }
            Decoded::Relayed { log, event } => {
                let relay_eth = pending_relay_eth.take();
                // Relays never nest: everything in the window since the
                // previous relay is either this relay's own execution or a
                // pull read. The window is consumed whatever happens to the
                // relay.
                let executing_candidates: Vec<ExecutingLeg> = std::mem::take(&mut window)
                    .into_iter()
                    .filter(|exec| exec.source_chain_id == event.source)
                    .collect();

                match classify_chain(event.source, configured_chains) {
                    ChainRef::Configured(source_chain_id) => {
                        if executing_candidates.is_empty() {
                            plan.observations.push(Observation::RelayUnpaired);
                        }
                        plan.actions.push(TxAction::Relay(DestinationLeg {
                            log,
                            source_chain_id,
                            nonce: event.messageNonce,
                            message_hash: event.messageHash,
                            return_data_hash: event.returnDataHash,
                            executing: None,
                            executing_candidates,
                            relay_eth,
                        }));
                    }
                    ChainRef::NotConfigured => plan
                        .observations
                        .push(Observation::Skipped(SkipReason::SourceNotConfigured)),
                    ChainRef::OutOfRange => plan
                        .observations
                        .push(Observation::Skipped(SkipReason::SourceOutOfRange)),
                }
            }
            Decoded::SendEth(event) => match pending_send_eth.take() {
                None => plan.observations.push(Observation::SendEthUnpaired),
                Some((transfer, destination)) => {
                    if event.from != transfer.from
                        || event.to != transfer.to
                        || event.amount != transfer.amount
                        || event.destination != destination
                    {
                        plan.observations.push(Observation::SendEthMismatch);
                    }
                }
            },
            Decoded::RelayEth(event) => {
                pending_relay_eth = Some(RelayEth {
                    from: event.from,
                    to: event.to,
                    amount: event.amount,
                    source: event.source,
                });
            }
        }
    }

    if pending_send_eth.is_some() {
        plan.observations.push(Observation::SendEthMissing);
    }

    Ok(plan)
}

pub(super) struct EventContext<'a> {
    pub(super) bridge_id: i32,
    pub(super) chain_id: ChainId,
    pub(super) registry: &'a OpInteropRegistry,
    /// The chains of this bridge; what the skip rules test.
    pub(super) configured_chains: &'a HashSet<ChainId>,
    pub(super) buffer: &'a Arc<MessageBuffer<Message>>,
}

/// Phase 1 plus its reporting: plans the transaction and emits the metrics
/// and logs of what it observed. Touches neither the buffer nor the database.
pub(super) fn plan_and_report(ctx: &EventContext<'_>, logs: &[&Log]) -> Result<TxPlan> {
    let plan = plan_transaction(ctx.registry, ctx.configured_chains, ctx.chain_id, logs)?;
    let tx_hash = logs.first().and_then(|log| log.transaction_hash);
    let block_number = logs.first().and_then(|log| log.block_number);

    for observation in &plan.observations {
        report_observation(ctx, *observation, tx_hash, block_number);
    }
    Ok(plan)
}

/// Phase 2: applies the actions to the buffer in order. A failing action does
/// not stop the rest; failures are aggregated into one `Err`.
///
/// A failing `alter` (its only fallible step is restoring the entry from
/// `pending_messages`) does not stop the remaining actions; the failures are
/// aggregated and returned once, as `xdai::events::dispatch_transaction`
/// does. Swallowing one would let the checkpoint move past a block whose
/// data was never applied.
pub(super) async fn apply_actions(
    ctx: &EventContext<'_>,
    actions: Vec<TxAction>,
    tx_hash: B256,
    block_number: Option<u64>,
) -> Result<()> {
    let mut last_err: Option<anyhow::Error> = None;
    let mut failed_actions = 0usize;

    for action in actions {
        if let Err(err) = apply_action(ctx, action).await {
            tracing::warn!(
                err = ?err,
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?tx_hash,
                block_number,
                "failed to apply OP interop message leg to the buffer"
            );
            failed_actions += 1;
            last_err = Some(err);
        }
    }

    match last_err {
        Some(err) => Err(err.context(format!(
            "{failed_actions} OP interop message leg(s) failed to apply"
        ))),
        None => Ok(()),
    }
}

async fn apply_action(ctx: &EventContext<'_>, action: TxAction) -> Result<()> {
    let (message_hash, log) = match &action {
        TxAction::Send(leg) => (leg.message_hash, leg.log.clone()),
        TxAction::Relay(leg) => (leg.message_hash, leg.log.clone()),
    };
    let key = key_from_message_hash(&message_hash, ctx.bridge_id)?;

    // The mutators cannot fail: every expected condition is an outcome.
    let mut outcome = None;
    ctx.buffer
        .alter(key, ctx.chain_id, log.block_number, |message| {
            outcome = Some(match action {
                TxAction::Send(leg) => message.apply_source(leg),
                TxAction::Relay(leg) => message.apply_destination(leg),
            });
            Ok(())
        })
        .await
        .map_err(|err| anyhow!(err))
        .with_context(|| {
            format!(
                "failed to alter the buffer entry of OP interop message {message_hash} \
                 (tx {}, log_index {})",
                log.transaction_hash, log.log_index
            )
        })?;

    let conflict = match outcome {
        Some(LegOutcome::DuplicateLeg) => "duplicate_leg",
        Some(LegOutcome::KeyCollision) => "key_collision",
        Some(LegOutcome::Set | LegOutcome::Unchanged) | None => return Ok(()),
    };
    metrics::OP_INTEROP_LEG_CONFLICTS_TOTAL
        .with_label_values(&[ctx.bridge_id.to_string().as_str(), conflict])
        .inc();
    tracing::warn!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        message_hash = %message_hash,
        message_id = key.message_id,
        tx_hash = %log.transaction_hash,
        block_number = log.block_number,
        block_hash = %log.block_hash,
        log_index = log.log_index,
        conflict,
        "OP interop log conflicts with the buffered message; keeping the first observation"
    );

    Ok(())
}

fn report_observation(
    ctx: &EventContext<'_>,
    observation: Observation,
    tx_hash: Option<alloy::primitives::B256>,
    block_number: Option<u64>,
) {
    let bridge = ctx.bridge_id.to_string();
    let chain = ctx.chain_id.to_string();

    match observation {
        Observation::Event(event) => {
            metrics::OP_INTEROP_EVENTS_TOTAL
                .with_label_values(&[bridge.as_str(), chain.as_str(), event.as_str()])
                .inc();
        }
        Observation::Skipped(reason) => {
            metrics::OP_INTEROP_SKIPPED_TOTAL
                .with_label_values(&[bridge.as_str(), chain.as_str(), reason.as_str()])
                .inc();
            tracing::trace!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?tx_hash,
                block_number,
                reason = reason.as_str(),
                "skipped an OP interop message whose other chain is not part of the bridge"
            );
        }
        Observation::Execution(origin) => {
            metrics::OP_INTEROP_RAW_EXECUTIONS_TOTAL
                .with_label_values(&[bridge.as_str(), chain.as_str(), origin.as_str()])
                .inc();
        }
        Observation::RelayUnpaired => {
            metrics::OP_INTEROP_RELAYS_UNPAIRED_TOTAL
                .with_label_values(&[bridge.as_str(), chain.as_str()])
                .inc();
            tracing::warn!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?tx_hash,
                block_number,
                "OP interop relay has no candidate ExecutingMessage in its relay window; \
                 the message is indexed without its executing_message detail"
            );
        }
        Observation::EthPayloadUnrecognized => {
            report_eth_anomaly(ctx, "unrecognized_payload", tx_hash, block_number)
        }
        Observation::SendEthMissing => {
            report_eth_anomaly(ctx, "send_eth_missing", tx_hash, block_number)
        }
        Observation::SendEthUnpaired => {
            report_eth_anomaly(ctx, "send_eth_unpaired", tx_hash, block_number)
        }
        Observation::SendEthMismatch => {
            report_eth_anomaly(ctx, "send_eth_mismatch", tx_hash, block_number)
        }
        Observation::WrongVersion => {
            crate::indexer::metrics::LOGS_DROPPED_WRONG_VERSION_TOTAL
                .with_label_values(&["op_interop", bridge.as_str(), chain.as_str()])
                .inc();
            tracing::warn!(
                bridge_id = ctx.bridge_id,
                chain_id = ctx.chain_id,
                tx_hash = ?tx_hash,
                block_number,
                "dropped an OP interop log whose topic belongs to a different configured \
                 version of this contract; check started_at_block for that address"
            );
        }
    }
}

fn report_eth_anomaly(
    ctx: &EventContext<'_>,
    kind: &'static str,
    tx_hash: Option<alloy::primitives::B256>,
    block_number: Option<u64>,
) {
    metrics::OP_INTEROP_ETH_TRANSFER_ANOMALIES_TOTAL
        .with_label_values(&[ctx.bridge_id.to_string().as_str(), kind])
        .inc();
    tracing::warn!(
        bridge_id = ctx.bridge_id,
        chain_id = ctx.chain_id,
        tx_hash = ?tx_hash,
        block_number,
        kind,
        "OP interop ETH transfer anomaly: the payload or the bridge events do not have the expected shape"
    );
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{Address, B256, b256};

    use super::{
        super::{
            test_fixtures::{
                DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, configured_chains, executing_message_log,
                foreign_read, pull_read, registry, relay_eth_log, relay_logs, relayed_message_log,
                send_calls, send_eth_log, sent_message_logs,
            },
            version::CROSS_L2_INBOX,
        },
        *,
    };

    const MESSAGE_0_HASH: B256 =
        b256!("274fa1e522d77c94a725679b973bd3070ac910488a6a5cf023c1469f94655eaa");
    const MESSAGE_1_HASH: B256 =
        b256!("56d7d57eb31833c6a0c545b97b72a4ddb4b7c4b6efac3252a8f1db21d96c203b");
    const MESSAGE_0_PAYLOAD_HASH: B256 =
        b256!("bb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb");

    fn plan(chain_id: ChainId, logs: &[Log]) -> Result<TxPlan> {
        let refs: Vec<&Log> = logs.iter().collect();
        plan_transaction(&registry(), &configured_chains(), chain_id, &refs)
    }

    fn with_log_index(mut log: Log, log_index: u64) -> Log {
        log.log_index = Some(log_index);
        log
    }

    /// Rewrites one 32-byte topic of a fixture log. The data is untouched, so
    /// this never encodes a payload with the declarations under test.
    fn with_topic(mut log: Log, position: usize, value: B256) -> Log {
        let mut topics = log.inner.data.topics().to_vec();
        topics[position] = value;
        log.inner.data = alloy::primitives::LogData::new_unchecked(topics, log.inner.data.data);
        log
    }

    /// Rewrites one 32-byte word of a fixture log's data.
    fn with_data_word(mut log: Log, word: usize, value: B256) -> Log {
        let mut data = log.inner.data.data.to_vec();
        data[word * 32..(word + 1) * 32].copy_from_slice(value.as_slice());
        log.inner.data = alloy::primitives::LogData::new_unchecked(
            log.inner.data.topics().to_vec(),
            data.into(),
        );
        log
    }

    fn word(value: U256) -> B256 {
        B256::from(value.to_be_bytes::<32>())
    }

    fn relays(plan: &TxPlan) -> Vec<&DestinationLeg> {
        plan.actions
            .iter()
            .map(|action| match action {
                TxAction::Relay(leg) => leg,
                other => panic!("expected a relay, got {other:?}"),
            })
            .collect()
    }

    fn sends(plan: &TxPlan) -> Vec<&SourceLeg> {
        plan.actions
            .iter()
            .map(|action| match action {
                TxAction::Send(leg) => leg,
                other => panic!("expected a send, got {other:?}"),
            })
            .collect()
    }

    fn count(plan: &TxPlan, observation: Observation) -> usize {
        plan.observations
            .iter()
            .filter(|seen| **seen == observation)
            .count()
    }

    #[test]
    fn test_plan_transaction_send_fixture_yields_two_sends() {
        let calls = send_calls();
        assert_eq!(calls.len(), 2);
        // The first call also holds a liquidity log and a bridge log.
        assert_eq!(calls[0].len(), 3);

        let first = plan(SOURCE_CHAIN_ID, &calls[0]).expect("first call plans");
        let second = plan(SOURCE_CHAIN_ID, &calls[1]).expect("second call plans");

        assert_eq!(first.actions.len(), 1);
        assert_eq!(
            first.observations,
            vec![
                Observation::Event(OpInteropEvent::SentMessage),
                Observation::Event(OpInteropEvent::SendEth),
            ],
            "the liquidity log leaves no trace; the paired SendETH raises no anomaly"
        );
        assert_eq!(second.actions.len(), 1);
        assert_eq!(
            second.observations,
            vec![Observation::Event(OpInteropEvent::SentMessage)]
        );

        let leg = sends(&first)[0];
        assert_eq!(leg.message_hash, MESSAGE_0_HASH);
        assert_eq!(leg.payload_hash, MESSAGE_0_PAYLOAD_HASH);
        assert_eq!(leg.destination_chain_id, DESTINATION_CHAIN_ID);
        assert_eq!(leg.nonce, U256::ZERO);
        let eth_bridge: Address = "0x4200000000000000000000000000000000000024"
            .parse()
            .unwrap();
        assert_eq!(leg.sender, eth_bridge);
        assert_eq!(leg.target, eth_bridge);
        assert_eq!(leg.message.len(), 100);
        assert_eq!(leg.log.chain_id, SOURCE_CHAIN_ID);
        assert_eq!(leg.log.block_number, 498_922);
        assert_eq!(
            leg.log.block_hash,
            b256!("8580247817bcfdd8f258ebd2f9a73aeddb4b87953114a723da65206fa002c8b5")
        );
        assert_eq!(leg.log.block_timestamp.and_utc().timestamp(), 1_794_670_098);
        assert_eq!(
            leg.log.transaction_hash,
            b256!("d7ff0cd43c35c6438f899a68526128c57b92995119a0900ea9493990ea8f5b9c")
        );
        assert_eq!(leg.log.log_index, 1);

        let leg = sends(&second)[0];
        assert_eq!(leg.message_hash, MESSAGE_1_HASH);
        assert_eq!(leg.nonce, U256::from(1));
        assert_eq!(leg.sender, Address::repeat_byte(0x11));
        assert_eq!(leg.target, Address::repeat_byte(0x22));
        assert_eq!(leg.message.as_ref(), [0xde, 0xad, 0xbe, 0xef]);
        assert_eq!(leg.log.log_index, 3);
        assert_eq!(
            leg.log.transaction_hash,
            b256!("de255f1dbf92bb1ff74a0c01fa96626360cf8cf159f47fbb3e0b6ce8dae3cecc")
        );
    }

    #[test]
    fn test_plan_transaction_relay_fixture_has_its_execution_as_the_candidate() {
        let logs = relay_logs();
        assert_eq!(logs.len(), 4, "inbox, liquidity, bridge, messenger");

        let plan = plan(DESTINATION_CHAIN_ID, &logs).expect("relay plans");

        assert_eq!(
            plan.observations,
            vec![
                Observation::Event(OpInteropEvent::ExecutingMessage),
                Observation::Execution(ExecutionOrigin::Messenger),
                Observation::Event(OpInteropEvent::RelayEth),
                Observation::Event(OpInteropEvent::RelayedMessage),
            ]
        );
        let leg = relays(&plan)[0];
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(leg.message_hash, MESSAGE_0_HASH);
        assert_eq!(leg.source_chain_id, SOURCE_CHAIN_ID);
        assert_eq!(leg.nonce, U256::ZERO);
        assert_eq!(
            leg.return_data_hash,
            b256!("c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
        );
        assert_eq!(leg.log.chain_id, DESTINATION_CHAIN_ID);
        assert_eq!(leg.log.block_number, 895_701);
        assert_eq!(
            leg.log.block_hash,
            b256!("7ea9e26557b70150ae7360da608f0f231e30397b4e5b4c98f4edd431d6630c73")
        );
        assert_eq!(leg.log.block_timestamp.and_utc().timestamp(), 1_794_670_100);
        assert_eq!(
            leg.log.transaction_hash,
            b256!("7155e2226f4cc4ffbb8987e756fa5e450a9c40c9e2e0dcd7f1195ae8aa58d03d")
        );
        assert_eq!(leg.log.log_index, 3);

        assert_eq!(leg.executing, None);
        assert_eq!(leg.executing_candidates.len(), 1);
        assert_eq!(plan.executions, leg.executing_candidates);
        let exec = &leg.executing_candidates[0];
        assert_eq!(exec.log.log_index, 0);
        assert_eq!(exec.log.block_hash, leg.log.block_hash);
        assert_eq!(exec.msg_hash, MESSAGE_0_PAYLOAD_HASH);
        assert_eq!(exec.origin, L2_TO_L2_MESSENGER);
        assert_eq!(exec.source_chain_id, U256::from(SOURCE_CHAIN_ID));
        assert_eq!(exec.source_block_number, U256::from(498_922));
        assert_eq!(exec.source_log_index, U256::from(1));
        assert_eq!(exec.source_timestamp, U256::from(1_794_670_098_u64));
    }

    /// `Exec A, Relayed A, Exec B, Relayed B`: each relay takes the
    /// `ExecutingMessage` of its own group. B is the fixture pair moved to
    /// later log indexes, with a different `msgHash` topic to tell it apart.
    #[test]
    fn test_plan_transaction_multicall_gives_each_relay_its_own_candidates() {
        let marker = B256::repeat_byte(0xB0);
        let logs = vec![
            executing_message_log(),
            relayed_message_log(),
            with_topic(with_log_index(executing_message_log(), 4), 1, marker),
            with_log_index(relayed_message_log(), 7),
        ];

        let plan = plan(DESTINATION_CHAIN_ID, &logs).expect("multicall plans");

        let relays = relays(&plan);
        assert_eq!(relays.len(), 2);
        assert_eq!(relays[0].executing_candidates.len(), 1);
        assert_eq!(relays[1].executing_candidates.len(), 1);
        let first = &relays[0].executing_candidates[0];
        let second = &relays[1].executing_candidates[0];
        assert_eq!((relays[0].log.log_index, first.log.log_index), (3, 0));
        assert_eq!(first.msg_hash, MESSAGE_0_PAYLOAD_HASH);
        assert_eq!((relays[1].log.log_index, second.log.log_index), (7, 4));
        assert_eq!(second.msg_hash, marker);
        assert_eq!(count(&plan, Observation::RelayUnpaired), 0);
        assert_eq!(plan.executions.len(), 2);
    }

    /// Someone reads the inbox directly between the messenger's
    /// `ExecutingMessage` and its `RelayedMessage`: not ours, and it must not
    /// take the relay's slot.
    #[test]
    fn test_plan_transaction_foreign_origin_executing_message_is_not_a_candidate() {
        let foreign = with_data_word(
            with_log_index(executing_message_log(), 1),
            0,
            B256::left_padding_from(Address::repeat_byte(0x99).as_slice()),
        );
        let logs = vec![executing_message_log(), foreign, relayed_message_log()];

        let plan = plan(DESTINATION_CHAIN_ID, &logs).expect("plans");

        let candidates = &relays(&plan)[0].executing_candidates;
        assert_eq!(candidates.len(), 1, "only the messenger's own execution");
        assert_eq!(candidates[0].log.log_index, 0);
        assert_eq!(candidates[0].origin, L2_TO_L2_MESSENGER);
        assert_eq!(
            count(&plan, Observation::Execution(ExecutionOrigin::Foreign)),
            1
        );
        assert_eq!(
            plan.executions
                .iter()
                .map(|exec| exec.log.log_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(count(&plan, Observation::RelayUnpaired), 0);
    }

    #[test]
    fn test_plan_transaction_relay_without_executing_message_is_unpaired_but_kept() {
        let plan =
            plan(DESTINATION_CHAIN_ID, &[relayed_message_log()]).expect("a lone relay plans");

        let relays = relays(&plan);
        assert_eq!(relays.len(), 1, "the relay is the fact");
        assert_eq!(relays[0].message_hash, MESSAGE_0_HASH);
        assert!(relays[0].executing_candidates.is_empty());
        assert_eq!(
            plan.observations,
            vec![
                Observation::Event(OpInteropEvent::RelayedMessage),
                Observation::RelayUnpaired,
            ]
        );

        // Only a foreign-origin ExecutingMessage before it: same outcome.
        let foreign = with_data_word(
            executing_message_log(),
            0,
            B256::left_padding_from(Address::repeat_byte(0x99).as_slice()),
        );
        let plan = plan_of_destination(&[foreign, relayed_message_log()]);
        assert!(self::relays(&plan)[0].executing_candidates.is_empty());
        assert_eq!(
            count(&plan, Observation::Execution(ExecutionOrigin::Foreign)),
            1
        );
        assert_eq!(count(&plan, Observation::RelayUnpaired), 1);
    }

    fn plan_of_destination(logs: &[Log]) -> TxPlan {
        plan(DESTINATION_CHAIN_ID, logs).expect("plans")
    }

    fn candidate_indexes(leg: &DestinationLeg) -> Vec<u64> {
        leg.executing_candidates
            .iter()
            .map(|exec| exec.log.log_index)
            .collect()
    }

    fn execution_indexes(plan: &TxPlan) -> Vec<u64> {
        plan.executions
            .iter()
            .map(|exec| exec.log.log_index)
            .collect()
    }

    #[test]
    fn test_plan_transaction_returns_every_execution_of_any_origin() {
        let plan = plan_of_destination(&relay_logs());
        assert_eq!(plan.executions.len(), 1);
        assert_eq!(plan.executions[0], relays(&plan)[0].executing_candidates[0]);

        let logs = vec![
            foreign_read(0),
            with_log_index(executing_message_log(), 1),
            pull_read(2, 7, B256::repeat_byte(0xA1)),
            with_log_index(relayed_message_log(), 5),
            pull_read(6, 8, B256::repeat_byte(0xA2)),
        ];
        let plan = plan_of_destination(&logs);
        assert_eq!(execution_indexes(&plan), vec![0, 1, 2, 6]);
        assert_eq!(
            count(&plan, Observation::Execution(ExecutionOrigin::Foreign)),
            1
        );
        assert_eq!(
            count(&plan, Observation::Execution(ExecutionOrigin::Messenger)),
            3
        );
        assert_eq!(candidate_indexes(relays(&plan)[0]), vec![1, 2]);
    }

    #[test]
    fn test_plan_transaction_relay_takes_every_candidate_of_its_window() {
        let marker = B256::repeat_byte(0xA0);

        // A pull read before the relay's own execution.
        let before = vec![
            pull_read(0, 7, marker),
            with_log_index(executing_message_log(), 1),
            with_log_index(relayed_message_log(), 3),
        ];
        let plan_before = plan_of_destination(&before);
        assert_eq!(candidate_indexes(relays(&plan_before)[0]), vec![0, 1]);

        // A pull read inside the target call.
        let inside = vec![
            executing_message_log(),
            pull_read(1, 7, marker),
            relayed_message_log(),
        ];
        let plan_inside = plan_of_destination(&inside);
        assert_eq!(candidate_indexes(relays(&plan_inside)[0]), vec![0, 1]);

        for plan in [&plan_before, &plan_inside] {
            assert_eq!(count(plan, Observation::RelayUnpaired), 0);
            assert_eq!(plan.executions.len(), 2);
        }
    }

    #[test]
    fn test_plan_transaction_skipped_relay_consumes_its_window() {
        let marker = B256::repeat_byte(0xA0);
        let logs = vec![
            pull_read(0, 7, marker),
            with_topic(
                with_log_index(relayed_message_log(), 1),
                1,
                word(U256::from(10)),
            ),
            with_log_index(executing_message_log(), 2),
            with_log_index(relayed_message_log(), 5),
        ];

        let plan = plan_of_destination(&logs);

        let relays = relays(&plan);
        assert_eq!(relays.len(), 1);
        assert_eq!(candidate_indexes(relays[0]), vec![2]);
        assert_eq!(
            count(&plan, Observation::Skipped(SkipReason::SourceNotConfigured)),
            1
        );
        assert_eq!(plan.executions.len(), 2);
    }

    /// The identifier names another source chain than the relay does: the
    /// `ExecutingMessage` is not this relay's.
    #[test]
    fn test_plan_transaction_execution_of_another_source_chain_is_not_a_candidate() {
        // `chainId` is the fifth word of the `Identifier`.
        let other_chain = with_data_word(
            executing_message_log(),
            4,
            word(U256::from(DESTINATION_CHAIN_ID)),
        );

        let plan = plan_of_destination(&[other_chain, relayed_message_log()]);

        let relays = relays(&plan);
        assert_eq!(relays.len(), 1);
        assert!(relays[0].executing_candidates.is_empty());
        assert_eq!(plan.executions.len(), 1);
        assert_eq!(count(&plan, Observation::RelayUnpaired), 1);
    }

    #[test]
    fn test_plan_transaction_truncated_log_is_an_error_with_no_actions() {
        let [first, second] = sent_message_logs();
        let mut truncated = second.clone();
        let data = truncated.inner.data.data.slice(..40);
        truncated.inner.data =
            alloy::primitives::LogData::new_unchecked(truncated.inner.data.topics().to_vec(), data);
        // Same transaction as the valid log, so the error is the whole
        // transaction's and the valid log yields no action either.
        truncated.transaction_hash = first.transaction_hash;

        let err = plan(SOURCE_CHAIN_ID, &[first.clone(), truncated])
            .expect_err("a truncated SentMessage cannot be decoded");
        assert!(
            format!("{err:#}").contains("failed to decode SentMessage"),
            "unexpected error: {err:#}"
        );

        // A relay whose indexed message hash is missing fails the same way.
        let mut short_relay = relayed_message_log();
        short_relay.inner.data = alloy::primitives::LogData::new_unchecked(
            short_relay.inner.data.topics()[..3].to_vec(),
            short_relay.inner.data.data.clone(),
        );
        assert!(
            plan(
                DESTINATION_CHAIN_ID,
                &[executing_message_log(), short_relay]
            )
            .is_err()
        );

        // So does a log row without its block timestamp.
        let mut no_timestamp = first;
        no_timestamp.block_timestamp = None;
        let err = plan(SOURCE_CHAIN_ID, &[no_timestamp]).expect_err("no timestamp");
        assert!(format!("{err:#}").contains("no block timestamp"));
    }

    #[test]
    fn test_plan_transaction_wrong_version_window_is_observed_and_dropped() {
        // Below the activation block the config says the contract did not
        // exist yet: its logs are dropped and counted, not decoded.
        let mut early = relay_logs();
        for log in &mut early {
            log.block_number = Some(829_212);
        }

        let plan = plan_of_destination(&early);

        assert!(plan.actions.is_empty());
        assert!(plan.executions.is_empty());
        assert_eq!(
            plan.observations,
            vec![
                Observation::WrongVersion,
                Observation::WrongVersion,
                Observation::WrongVersion
            ]
        );
    }

    #[test]
    fn test_plan_transaction_skips_unconfigured_and_out_of_range_endpoints() {
        // The plain message: an ETH send would also expect its `SendETH`.
        let [_, sent] = sent_message_logs();

        // Destination topic is the first indexed parameter of `SentMessage`.
        let unconfigured = with_topic(sent.clone(), 1, word(U256::from(10)));
        let plan_unconfigured = plan(SOURCE_CHAIN_ID, &[unconfigured]).expect("plans");
        assert!(plan_unconfigured.actions.is_empty());
        assert_eq!(
            plan_unconfigured.observations,
            vec![
                Observation::Event(OpInteropEvent::SentMessage),
                Observation::Skipped(SkipReason::DestinationNotConfigured),
            ]
        );

        let out_of_range = with_topic(sent, 1, word(U256::from(1) << 200));
        let plan_out_of_range = plan(SOURCE_CHAIN_ID, &[out_of_range]).expect("plans");
        assert!(plan_out_of_range.actions.is_empty());
        assert_eq!(
            count(
                &plan_out_of_range,
                Observation::Skipped(SkipReason::DestinationOutOfRange)
            ),
            1
        );

        // Source topic is the first indexed parameter of `RelayedMessage`.
        // The skipped relay drops its candidate, but the execution is still a
        // raw execution.
        let relayed_unconfigured = with_topic(relayed_message_log(), 1, word(U256::from(10)));
        let plan = plan_of_destination(&[executing_message_log(), relayed_unconfigured]);
        assert!(plan.actions.is_empty());
        assert_eq!(
            count(&plan, Observation::Skipped(SkipReason::SourceNotConfigured)),
            1
        );
        assert_eq!(plan.executions.len(), 1);
        assert_eq!(count(&plan, Observation::RelayUnpaired), 0);

        let relayed_out_of_range = with_topic(relayed_message_log(), 1, word(U256::MAX));
        let plan = plan_of_destination(&[relayed_out_of_range]);
        assert!(plan.actions.is_empty());
        assert_eq!(
            count(&plan, Observation::Skipped(SkipReason::SourceOutOfRange)),
            1
        );
    }

    #[test]
    fn test_plan_transaction_ignores_logs_of_unconfigured_chain_or_without_topics() {
        // The same logs scanned on a chain the registry does not know.
        let plan = plan(10, &relay_logs()).expect("plans");
        assert_eq!(plan, TxPlan::default());

        let mut no_topics = executing_message_log();
        no_topics.inner.data = alloy::primitives::LogData::new_unchecked(
            Vec::new(),
            no_topics.inner.data.data.clone(),
        );
        assert_eq!(no_topics.address(), CROSS_L2_INBOX);
        let plan = plan_of_destination(&[no_topics]);
        assert_eq!(plan, TxPlan::default());
    }

    fn eth_send_pair() -> [Log; 2] {
        let [sent, _] = sent_message_logs();
        [sent, send_eth_log()]
    }

    /// Rewrites one byte of a fixture log's data.
    fn with_data_byte(mut log: Log, index: usize, value: u8) -> Log {
        let mut data = log.inner.data.data.to_vec();
        data[index] = value;
        log.inner.data = alloy::primitives::LogData::new_unchecked(
            log.inner.data.topics().to_vec(),
            data.into(),
        );
        log
    }

    const ETH_ANOMALIES: [Observation; 4] = [
        Observation::EthPayloadUnrecognized,
        Observation::SendEthMissing,
        Observation::SendEthUnpaired,
        Observation::SendEthMismatch,
    ];

    fn eth_anomalies(plan: &TxPlan) -> Vec<Observation> {
        plan.observations
            .iter()
            .copied()
            .filter(|observation| ETH_ANOMALIES.contains(observation))
            .collect()
    }

    #[test]
    fn test_plan_transaction_eth_send_fixture_has_no_anomaly() {
        let calls = send_calls();
        let plan = plan(SOURCE_CHAIN_ID, &calls[0]).expect("plans");
        assert!(eth_anomalies(&plan).is_empty());
        assert_eq!(count(&plan, Observation::Event(OpInteropEvent::SendEth)), 1);
    }

    #[test]
    fn test_plan_transaction_eth_send_without_send_eth_is_missing() {
        let [sent, _] = eth_send_pair();
        let plan = plan(SOURCE_CHAIN_ID, &[sent]).expect("plans");
        assert_eq!(plan.actions.len(), 1, "the message is indexed regardless");
        assert_eq!(eth_anomalies(&plan), vec![Observation::SendEthMissing]);
    }

    #[test]
    fn test_plan_transaction_send_eth_that_disagrees_is_mismatch() {
        let [sent, send_eth] = eth_send_pair();
        let other = U256::from(2);
        for edited in [
            with_data_word(send_eth.clone(), 0, word(other)),
            with_topic(send_eth.clone(), 2, word(U256::from(0x33))),
            with_topic(send_eth.clone(), 1, word(U256::from(0x33))),
            with_data_word(send_eth, 1, word(U256::from(SOURCE_CHAIN_ID))),
        ] {
            let plan = plan(SOURCE_CHAIN_ID, &[sent.clone(), edited]).expect("plans");
            assert_eq!(plan.actions.len(), 1);
            assert_eq!(eth_anomalies(&plan), vec![Observation::SendEthMismatch]);
        }
    }

    #[test]
    fn test_plan_transaction_send_eth_alone_is_unpaired() {
        let plan = plan(SOURCE_CHAIN_ID, &[send_eth_log()]).expect("plans");
        assert!(plan.actions.is_empty());
        assert_eq!(eth_anomalies(&plan), vec![Observation::SendEthUnpaired]);
    }

    #[test]
    fn test_plan_transaction_two_eth_sends_yield_two_sends_without_anomaly() {
        let [sent, send_eth] = eth_send_pair();
        let logs = vec![
            sent.clone(),
            send_eth.clone(),
            with_log_index(sent, 3),
            with_log_index(send_eth, 4),
        ];
        let plan = plan(SOURCE_CHAIN_ID, &logs).expect("plans");
        assert_eq!(sends(&plan).len(), 2);
        assert!(eth_anomalies(&plan).is_empty());
    }

    #[test]
    fn test_plan_transaction_eth_send_to_unconfigured_chain_is_skipped_without_anomaly() {
        let [sent, send_eth] = eth_send_pair();
        let unconfigured = word(U256::from(10));
        let logs = vec![
            with_topic(sent, 1, unconfigured),
            with_data_word(send_eth, 1, unconfigured),
        ];
        let plan = plan(SOURCE_CHAIN_ID, &logs).expect("plans");
        assert!(plan.actions.is_empty());
        assert_eq!(
            count(
                &plan,
                Observation::Skipped(SkipReason::DestinationNotConfigured)
            ),
            1
        );
        assert!(eth_anomalies(&plan).is_empty());
    }

    #[test]
    fn test_plan_transaction_unrecognized_bridge_payload_keeps_send_and_orphans_send_eth() {
        let [sent, send_eth] = eth_send_pair();
        // The selector starts the fourth data word: sender, offset, length.
        let edited = with_data_byte(sent, 96, 0xff);
        let plan = plan(SOURCE_CHAIN_ID, &[edited, send_eth]).expect("plans");
        assert_eq!(plan.actions.len(), 1, "the Send action is kept");
        assert_eq!(
            eth_anomalies(&plan),
            vec![
                Observation::EthPayloadUnrecognized,
                Observation::SendEthUnpaired
            ]
        );
    }

    #[test]
    fn test_plan_transaction_relay_fixture_carries_relay_eth() {
        let plan = plan_of_destination(&relay_logs());
        let relay_eth = relays(&plan)[0].relay_eth.as_ref().expect("relay_eth");
        assert_eq!(relay_eth.from, Address::repeat_byte(0x11));
        assert_eq!(relay_eth.to, Address::repeat_byte(0x22));
        assert_eq!(relay_eth.amount, U256::from(1_000_000_000_000_000_000_u64));
        assert_eq!(relay_eth.source, U256::from(SOURCE_CHAIN_ID));

        let without: Vec<Log> = relay_logs()
            .into_iter()
            .filter(|log| log.address() != relay_eth_log().address())
            .collect();
        let plan = plan_of_destination(&without);
        assert_eq!(relays(&plan)[0].relay_eth, None);
    }

    #[test]
    fn test_plan_transaction_multicall_gives_each_relay_its_own_relay_eth() {
        let second_amount = word(U256::from(5));
        let logs = vec![
            executing_message_log(),
            relay_eth_log(),
            relayed_message_log(),
            with_log_index(executing_message_log(), 4),
            with_data_word(with_log_index(relay_eth_log(), 5), 0, second_amount),
            with_log_index(relayed_message_log(), 7),
        ];
        let plan = plan_of_destination(&logs);
        let relays = relays(&plan);
        assert_eq!(relays.len(), 2);
        assert_eq!(
            relays[0].relay_eth.as_ref().map(|relay| relay.amount),
            Some(U256::from(1_000_000_000_000_000_000_u64))
        );
        assert_eq!(
            relays[1].relay_eth.as_ref().map(|relay| relay.amount),
            Some(U256::from(5))
        );
    }

    #[test]
    fn test_plan_transaction_relay_eth_does_not_leak_into_the_next_group() {
        let logs = vec![
            executing_message_log(),
            relay_eth_log(),
            with_log_index(executing_message_log(), 3),
            with_log_index(relayed_message_log(), 7),
        ];
        let plan = plan_of_destination(&logs);
        assert_eq!(relays(&plan)[0].relay_eth, None);
    }

    #[test]
    fn test_plan_transaction_truncated_send_eth_is_an_error_with_no_actions() {
        let [sent, send_eth] = eth_send_pair();
        let mut truncated = send_eth;
        let data = truncated.inner.data.data.slice(..32);
        truncated.inner.data =
            alloy::primitives::LogData::new_unchecked(truncated.inner.data.topics().to_vec(), data);
        let err = plan(SOURCE_CHAIN_ID, &[sent, truncated])
            .expect_err("a truncated SendETH cannot be decoded");
        assert!(
            format!("{err:#}").contains("failed to decode SendETH"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn test_classify_chain_boundaries() {
        let configured = configured_chains();

        assert_eq!(
            classify_chain(U256::from(SOURCE_CHAIN_ID), &configured),
            ChainRef::Configured(SOURCE_CHAIN_ID)
        );
        assert_eq!(
            classify_chain(U256::from(DESTINATION_CHAIN_ID), &configured),
            ChainRef::Configured(DESTINATION_CHAIN_ID)
        );
        assert_eq!(
            classify_chain(U256::from(10), &configured),
            ChainRef::NotConfigured
        );
        assert_eq!(
            classify_chain(U256::ZERO, &configured),
            ChainRef::NotConfigured
        );
        assert_eq!(
            classify_chain(U256::from(u64::MAX), &configured),
            ChainRef::NotConfigured,
            "the largest u64 is in range, just not configured"
        );
        assert_eq!(
            classify_chain(U256::from(1) << 64, &configured),
            ChainRef::OutOfRange
        );
        assert_eq!(classify_chain(U256::MAX, &configured), ChainRef::OutOfRange);
    }

    #[test]
    fn test_log_ref_names_the_missing_field() {
        let complete = relayed_message_log();
        assert!(log_ref(DESTINATION_CHAIN_ID, &complete).is_ok());

        type Break = fn(&mut Log);
        let cases: [(Break, &str); 6] = [
            (|log| log.block_number = None, "no block number"),
            (|log| log.block_hash = None, "no block hash"),
            (|log| log.transaction_hash = None, "no transaction hash"),
            (|log| log.log_index = None, "no log index"),
            (|log| log.block_timestamp = None, "no block timestamp"),
            (|log| log.removed = true, "marked as removed"),
        ];
        for (break_log, expected) in cases {
            let mut log = complete.clone();
            break_log(&mut log);
            let err = log_ref(DESTINATION_CHAIN_ID, &log).expect_err("incomplete row rejected");
            assert!(
                err.to_string().contains(expected),
                "expected {expected:?} in {err}"
            );
        }

        let mut far_future = complete.clone();
        far_future.block_number = Some(u64::MAX);
        assert!(log_ref(DESTINATION_CHAIN_ID, &far_future).is_err());
        let mut bad_time = complete;
        bad_time.block_timestamp = Some(u64::MAX);
        assert!(log_ref(DESTINATION_CHAIN_ID, &bad_time).is_err());
    }
}
