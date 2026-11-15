// SPDX-License-Identifier: LicenseRef-Blockscout

use alloy::{
    primitives::{Address, B256, Bytes, ChainId, U256, keccak256},
    sol_types::SolValue,
};
use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::message_buffer::Key;

/// Where a log was observed. Everything here comes from the `eth_getLogs`
/// row itself: this indexer fetches no receipt and no block header.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LogRef {
    pub(crate) chain_id: ChainId,
    pub(crate) block_number: u64,
    pub(crate) block_hash: B256,
    pub(crate) block_timestamp: NaiveDateTime,
    pub(crate) transaction_hash: B256,
    /// Block-level log index.
    pub(crate) log_index: u64,
}

impl LogRef {
    /// Block hash plus block-level index identifies one log.
    fn is_same_log(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash && self.log_index == other.log_index
    }
}

/// The `SentMessage` half of a message, observed on the source chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SourceLeg {
    pub(crate) log: LogRef,
    pub(crate) destination_chain_id: ChainId,
    /// Full versioned `uint256` nonce as emitted.
    pub(crate) nonce: U256,
    /// `msg.sender` of `sendMessage` -- a contract for most traffic, and
    /// never the transaction origin (no transaction is fetched).
    pub(crate) sender: Address,
    pub(crate) target: Address,
    pub(crate) message: Bytes,
    /// The identity of the message; see [`message_hash`].
    pub(crate) message_hash: B256,
    /// Hash of this `SentMessage` *log*; see [`payload_hash`]. An attribute
    /// and a cross-check input, never a key.
    pub(crate) payload_hash: B256,
}

/// The `CrossL2Inbox.ExecutingMessage` log a relay was paired with: the
/// on-chain `Identifier` verbatim, plus where the log itself was seen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExecutingLeg {
    pub(crate) log: LogRef,
    /// `ExecutingMessage.msgHash`: the payload hash of the source log, **not**
    /// the message hash.
    pub(crate) msg_hash: B256,
    pub(crate) origin: Address,
    pub(crate) source_chain_id: U256,
    pub(crate) source_block_number: U256,
    pub(crate) source_log_index: U256,
    pub(crate) source_timestamp: U256,
}

/// The `RelayedMessage` half of a message, observed on the destination chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DestinationLeg {
    pub(crate) log: LogRef,
    pub(crate) source_chain_id: ChainId,
    pub(crate) nonce: U256,
    /// The event's third topic.
    pub(crate) message_hash: B256,
    pub(crate) return_data_hash: B256,
    /// `None` for a relay with no usable `ExecutingMessage` before it in its
    /// transaction. The relay is the fact; the identifier is an attribute.
    pub(crate) executing: Option<ExecutingLeg>,
}

/// Persisted as JSON in `pending_messages.payload`. Any field added later
/// must be `#[serde(default)]` and come with a test that an old-form payload
/// still deserializes: a payload that fails to deserialize fails the block
/// that touched it, forever.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) source: Option<SourceLeg>,
    pub(crate) destination: Option<DestinationLeg>,
}

/// What applying a leg did. Returned, not raised: none of these is an error,
/// and the caller decides what to count and log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LegOutcome {
    /// The slot was empty; the leg is stored.
    Set,
    /// The slot already held this very log; it is overwritten with the
    /// (identical) leg. A replayed block.
    Unchanged,
    /// The slot holds a different log for the same side; the first is kept.
    DuplicateLeg,
    /// The entry already holds a leg with a different message hash: two
    /// messages share one 64-bit buffer key. The first is kept.
    KeyCollision,
}

impl Message {
    /// The message hash of whichever leg the entry already holds.
    fn held_message_hash(&self) -> Option<B256> {
        self.source
            .as_ref()
            .map(|source| source.message_hash)
            .or_else(|| {
                self.destination
                    .as_ref()
                    .map(|destination| destination.message_hash)
            })
    }

    fn collides_with(&self, message_hash: B256) -> bool {
        self.held_message_hash()
            .is_some_and(|held| held != message_hash)
    }

    pub(crate) fn apply_source(&mut self, leg: SourceLeg) -> LegOutcome {
        if self.collides_with(leg.message_hash) {
            return LegOutcome::KeyCollision;
        }
        match &self.source {
            None => {
                self.source = Some(leg);
                LegOutcome::Set
            }
            Some(existing) if existing.log.is_same_log(&leg.log) => {
                self.source = Some(leg);
                LegOutcome::Unchanged
            }
            Some(_) => LegOutcome::DuplicateLeg,
        }
    }

    pub(crate) fn apply_destination(&mut self, leg: DestinationLeg) -> LegOutcome {
        if self.collides_with(leg.message_hash) {
            return LegOutcome::KeyCollision;
        }
        match &self.destination {
            None => {
                self.destination = Some(leg);
                LegOutcome::Set
            }
            Some(existing) if existing.log.is_same_log(&leg.log) => {
                self.destination = Some(leg);
                LegOutcome::Unchanged
            }
            Some(_) => LegOutcome::DuplicateLeg,
        }
    }
}

/// The messenger's message hash, the identity of a message:
/// `keccak256(abi.encode(destination, source, nonce, sender, target, message))`
/// with Solidity types `(uint256, uint256, uint256, address, address, bytes)`.
///
/// `source` is the chain id of the chain that emitted `SentMessage`; it is not
/// in the event. `RelayedMessage` carries the result as its third topic.
pub(crate) fn message_hash(
    destination: U256,
    source: U256,
    nonce: U256,
    sender: Address,
    target: Address,
    message: &[u8],
) -> B256 {
    keccak256(
        (
            destination,
            source,
            nonce,
            sender,
            target,
            Bytes::copy_from_slice(message),
        )
            .abi_encode_params(),
    )
}

/// The hash `CrossL2Inbox` checks an executing message against:
/// `keccak256(topic0 ‖ topic1 ‖ … ‖ data)` of the initiating log. For a
/// messenger relay this is `ExecutingMessage.msgHash`.
///
/// Not the message hash. Never a key.
pub(crate) fn payload_hash(log: &alloy::primitives::Log) -> B256 {
    let topics = log.data.topics();
    let mut preimage = Vec::with_capacity(topics.len() * 32 + log.data.data.len());
    for topic in topics {
        preimage.extend_from_slice(topic.as_slice());
    }
    preimage.extend_from_slice(&log.data.data);
    keccak256(preimage)
}

/// Buffer key for an OP interop message: the first 8 bytes, big-endian, of
/// `keccak256(message_hash)`. The same derivation the AMB and xDai indexers
/// use for their own identities; each protocol keeps its own copy.
pub(crate) fn key_from_message_hash(message_hash: &B256, bridge_id: i32) -> Result<Key> {
    let digest = keccak256(message_hash);
    let bytes: [u8; 8] = digest.as_slice()[..8].try_into()?;
    Ok(Key::new(
        i64::from_be_bytes(bytes),
        i16::try_from(bridge_id).context("bridge_id out of range")?,
    ))
}

#[cfg(test)]
mod tests {
    use alloy::primitives::{b256, bytes};

    use super::{
        super::{
            abi::v1,
            test_fixtures::{
                DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, executing_message_log, relayed_message_log,
                sent_message_logs,
            },
        },
        *,
    };

    const MESSAGE_0_HASH: B256 =
        b256!("274fa1e522d77c94a725679b973bd3070ac910488a6a5cf023c1469f94655eaa");
    const MESSAGE_1_HASH: B256 =
        b256!("56d7d57eb31833c6a0c545b97b72a4ddb4b7c4b6efac3252a8f1db21d96c203b");
    const MESSAGE_0_PAYLOAD_HASH: B256 =
        b256!("bb39b9f3ce34b43c1ae2bcc42aa7a2ceb451b20cdd22f2396d2ea6ce16bf31fb");

    fn hash_of_sent_message(log: &alloy::rpc::types::Log) -> B256 {
        let event = log
            .log_decode::<v1::SentMessage>()
            .expect("fixture SentMessage decodes")
            .inner
            .data;
        message_hash(
            event.destination,
            U256::from(SOURCE_CHAIN_ID),
            event.messageNonce,
            event.sender,
            event.target,
            &event.message,
        )
    }

    #[test]
    fn test_message_hash_matches_devnet_vectors() {
        let [first, second] = sent_message_logs();

        assert_eq!(hash_of_sent_message(&first), MESSAGE_0_HASH);
        assert_eq!(hash_of_sent_message(&second), MESSAGE_1_HASH);

        // The relay names the same message: third topic of `RelayedMessage`.
        let relayed = relayed_message_log();
        assert_eq!(relayed.topics()[3], MESSAGE_0_HASH);

        // The vector spelled out, independent of the event decoder.
        assert_eq!(
            message_hash(
                U256::from(DESTINATION_CHAIN_ID),
                U256::from(SOURCE_CHAIN_ID),
                U256::from(1),
                Address::repeat_byte(0x11),
                Address::repeat_byte(0x22),
                &bytes!("deadbeef"),
            ),
            MESSAGE_1_HASH
        );
    }

    #[test]
    fn test_payload_hash_matches_executing_message_msg_hash() {
        let [first, second] = sent_message_logs();

        assert_eq!(payload_hash(&first.inner), MESSAGE_0_PAYLOAD_HASH);
        // `ExecutingMessage.msgHash` is its first indexed parameter.
        assert_eq!(executing_message_log().topics()[1], MESSAGE_0_PAYLOAD_HASH);
        // The two hashes of one message are different things.
        assert_ne!(payload_hash(&first.inner), MESSAGE_0_HASH);
        assert_ne!(payload_hash(&second.inner), payload_hash(&first.inner));
    }

    #[test]
    fn test_key_from_message_hash_vectors() {
        assert_eq!(
            key_from_message_hash(&MESSAGE_0_HASH, 2004).unwrap(),
            Key::new(-8172949633179243438, 2004)
        );
        assert_eq!(
            key_from_message_hash(&MESSAGE_1_HASH, 2004).unwrap(),
            Key::new(-5851365123218774601, 2004)
        );
        assert!(
            key_from_message_hash(&MESSAGE_0_HASH, i32::from(i16::MAX) + 1).is_err(),
            "a bridge id outside the buffer key's range is rejected"
        );
    }

    fn log_ref(chain_id: ChainId, block_byte: u8, log_index: u64) -> LogRef {
        LogRef {
            chain_id,
            block_number: 100 + u64::from(block_byte),
            block_hash: B256::repeat_byte(block_byte),
            block_timestamp: chrono::DateTime::from_timestamp(1_794_670_098, 0)
                .unwrap()
                .naive_utc(),
            transaction_hash: B256::repeat_byte(block_byte.wrapping_add(1)),
            log_index,
        }
    }

    fn source_leg(message_hash: B256, block_byte: u8, log_index: u64) -> SourceLeg {
        SourceLeg {
            log: log_ref(SOURCE_CHAIN_ID, block_byte, log_index),
            destination_chain_id: DESTINATION_CHAIN_ID,
            nonce: U256::from(7),
            sender: Address::repeat_byte(0x11),
            target: Address::repeat_byte(0x22),
            message: bytes!("deadbeef"),
            message_hash,
            payload_hash: B256::repeat_byte(0xEE),
        }
    }

    fn destination_leg(message_hash: B256, block_byte: u8, log_index: u64) -> DestinationLeg {
        DestinationLeg {
            log: log_ref(DESTINATION_CHAIN_ID, block_byte, log_index),
            source_chain_id: SOURCE_CHAIN_ID,
            nonce: U256::from(7),
            message_hash,
            return_data_hash: B256::repeat_byte(0xDD),
            executing: Some(ExecutingLeg {
                log: log_ref(
                    DESTINATION_CHAIN_ID,
                    block_byte,
                    log_index.saturating_sub(1),
                ),
                msg_hash: B256::repeat_byte(0xEE),
                origin: Address::repeat_byte(0x23),
                source_chain_id: U256::from(SOURCE_CHAIN_ID),
                source_block_number: U256::from(101),
                source_log_index: U256::from(1),
                source_timestamp: U256::from(1_794_670_098_u64),
            }),
        }
    }

    #[test]
    fn test_message_round_trips_through_json() {
        let mut message = Message::default();
        message.apply_source(source_leg(MESSAGE_0_HASH, 1, 1));
        message.apply_destination(destination_leg(MESSAGE_0_HASH, 2, 3));

        let json = serde_json::to_value(&message).expect("serializes");
        let restored: Message = serde_json::from_value(json).expect("deserializes");

        assert_eq!(restored.source, message.source);
        assert_eq!(restored.destination, message.destination);

        // An empty entry and a one-sided one round-trip as well.
        let empty: Message =
            serde_json::from_value(serde_json::to_value(Message::default()).unwrap()).unwrap();
        assert!(empty.source.is_none() && empty.destination.is_none());

        let mut destination_only = Message::default();
        let mut unpaired = destination_leg(MESSAGE_0_HASH, 2, 3);
        unpaired.executing = None;
        destination_only.apply_destination(unpaired.clone());
        let restored: Message =
            serde_json::from_value(serde_json::to_value(&destination_only).unwrap()).unwrap();
        assert_eq!(restored.source, None);
        assert_eq!(restored.destination, Some(unpaired));
    }

    #[test]
    fn test_apply_leg_outcomes() {
        // Source side.
        let mut message = Message::default();
        assert_eq!(
            message.apply_source(source_leg(MESSAGE_0_HASH, 1, 1)),
            LegOutcome::Set
        );
        assert_eq!(
            message.apply_source(source_leg(MESSAGE_0_HASH, 1, 1)),
            LegOutcome::Unchanged,
            "the same log again"
        );
        assert_eq!(
            message.apply_source(source_leg(MESSAGE_0_HASH, 9, 1)),
            LegOutcome::DuplicateLeg,
            "a different block hash is a different log"
        );
        assert_eq!(
            message.apply_source(source_leg(MESSAGE_0_HASH, 1, 2)),
            LegOutcome::DuplicateLeg,
            "a different log index is a different log"
        );
        assert_eq!(
            message.apply_source(source_leg(MESSAGE_1_HASH, 1, 1)),
            LegOutcome::KeyCollision
        );
        assert_eq!(
            message.source,
            Some(source_leg(MESSAGE_0_HASH, 1, 1)),
            "the first source leg is kept through every conflict"
        );

        // Destination side, on the entry that already holds the source.
        assert_eq!(
            message.apply_destination(destination_leg(MESSAGE_1_HASH, 2, 3)),
            LegOutcome::KeyCollision,
            "a destination for another message must not join this source"
        );
        assert_eq!(message.destination, None);
        assert_eq!(
            message.apply_destination(destination_leg(MESSAGE_0_HASH, 2, 3)),
            LegOutcome::Set
        );
        assert_eq!(
            message.apply_destination(destination_leg(MESSAGE_0_HASH, 2, 3)),
            LegOutcome::Unchanged
        );
        assert_eq!(
            message.apply_destination(destination_leg(MESSAGE_0_HASH, 8, 3)),
            LegOutcome::DuplicateLeg
        );
        assert_eq!(
            message.destination,
            Some(destination_leg(MESSAGE_0_HASH, 2, 3))
        );

        // Destination first: the source of another message collides.
        let mut destination_first = Message::default();
        assert_eq!(
            destination_first.apply_destination(destination_leg(MESSAGE_0_HASH, 2, 3)),
            LegOutcome::Set
        );
        assert_eq!(
            destination_first.apply_source(source_leg(MESSAGE_1_HASH, 1, 1)),
            LegOutcome::KeyCollision
        );
        assert_eq!(destination_first.source, None);
        assert_eq!(
            destination_first.apply_source(source_leg(MESSAGE_0_HASH, 1, 1)),
            LegOutcome::Set
        );
    }
}
