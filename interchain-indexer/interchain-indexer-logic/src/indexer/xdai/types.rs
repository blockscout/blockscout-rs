use std::collections::HashMap;

use alloy::primitives::{Address, B256, U256, keccak256};
use anyhow::{Context, Result, ensure};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::message_buffer::Key;

/// Sentinel written for the Gnosis leg of a transfer, which is always native
/// xDAI and therefore has no token contract to record. A named constant, not
/// a literal at each call site: if another bridge later writes a native leg
/// on chain 100, both must use this exact value or they form two disjoint
/// `stats_assets` rows for the same coin. See the gotcha in
/// `.memory-bank/gotchas.md` for the full rationale, including why the
/// matching `tokens` row must be seeded.
pub(crate) const NATIVE_SENTINEL: Address = Address::ZERO;

/// The nonce is a **per-contract** monotonic counter, so Eth→Gno and
/// Gno→Eth have independent, overlapping ranges. `initiator_chain_id` is
/// what makes identity globally unique — never key on the bare nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Direction {
    EthToGno,
    GnoToEth,
}

impl Direction {
    pub(crate) fn initiator_chain_id(self) -> i64 {
        match self {
            Direction::EthToGno => 1,
            Direction::GnoToEth => 100,
        }
    }

    pub(crate) fn destination_chain_id(self) -> i64 {
        match self {
            Direction::EthToGno => 100,
            Direction::GnoToEth => 1,
        }
    }
}

/// `initiator_chain_id (4 B, BE) ‖ nonce (28 B, BE)` — the 32-byte blob the
/// official bridge explorer keys transactions on. Byte-identical to what
/// `bridge.gnosischain.com/bridge-explorer` uses, so writing it to
/// `crosschain_messages.native_id` makes the existing `{{message_id}}`
/// `ui_url` template resolve with no serving-layer change.
///
/// A nonce that does not fit in 28 bytes is an unrepresentable identity
/// (nowhere near a real deployment's counter) and must fail loudly rather
/// than silently truncate.
pub(crate) fn native_id_blob(initiator_chain_id: i64, nonce: U256) -> Result<[u8; 32]> {
    let nonce_bytes = nonce.to_be_bytes::<32>();
    ensure!(
        nonce_bytes[0..4] == [0u8; 4],
        "xDai nonce {nonce} does not fit in the 28-byte native_id encoding"
    );

    let mut blob = [0u8; 32];
    blob[0..4].copy_from_slice(&(initiator_chain_id as u32).to_be_bytes());
    blob[4..32].copy_from_slice(&nonce_bytes[4..32]);
    Ok(blob)
}

/// Buffer key for an xDai message: the first 8 bytes of `keccak256(native_id)`.
/// Mirrors `amb/events.rs::key_from_message_id` for the same reason — the low
/// bytes of the raw blob are a small monotonic counter and would cluster in
/// the `i64` space without the hash.
pub(crate) fn key_from_native_id(native_id: &[u8; 32], bridge_id: i32) -> Result<Key> {
    let digest = keccak256(native_id);
    let bytes: [u8; 8] = digest.as_slice()[..8].try_into()?;
    Ok(Key::new(
        i64::from_be_bytes(bytes),
        i16::try_from(bridge_id).context("bridge_id out of range")?,
    ))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AnnotatedEvent<T> {
    pub(crate) event: T,
    pub(crate) transaction_hash: B256,
    pub(crate) block_number: i64,
    pub(crate) block_timestamp: NaiveDateTime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct UserRequestForAffirmationEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
    pub(crate) nonce: U256,
    /// The Ethereum-side asset resolved from the grammar table at the log's
    /// own block/version (DAI below Foreign v10, USDS from it) — never from
    /// a log, since `UserRequestForAffirmation` carries no token field, and
    /// never from a `latest`-block RPC call, which would relabel history.
    pub(crate) source_asset: Address,
}

/// Payload shared by both destination-completion events
/// (`AffirmationCompleted` and `RelayedMessage`): each carries only
/// `(recipient, value)` beyond the nonce, which is already known from the
/// source event and not re-stored here.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CompletionEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
}

/// Which destination event produced a [`CompletionEvent`]. Mirrors
/// `amb::types::DestinationExecution`: the variant records provenance: an
/// `Affirmation` completes an Eth→Gno message, a `Relayed` completes a
/// Gno→Eth one. Nothing enforces that pairing at this type's level — the
/// direction check in `consolidation.rs::status_and_finality` is what
/// actually keeps a `CollectedSignatures` that resolved onto the wrong key
/// from producing `ReadyToClaim`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum Completion {
    Affirmation(AnnotatedEvent<CompletionEvent>),
    Relayed(AnnotatedEvent<CompletionEvent>),
}

impl Completion {
    pub(crate) fn event(&self) -> &AnnotatedEvent<CompletionEvent> {
        match self {
            Self::Affirmation(event) | Self::Relayed(event) => event,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct UserRequestForSignatureEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
    pub(crate) nonce: U256,
    /// Destination-side asset, explicit only from Home v7. `None` below v7
    /// means DAI — the legacy 104-byte message layout hardcodes it.
    pub(crate) token: Option<Address>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct CollectedSignaturesEvent {
    pub(crate) authority_responsible_for_relay: Address,
    pub(crate) message_hash: B256,
    /// `requiredSignatures()`, **not** the number actually collected — do
    /// not treat it as a tally.
    pub(crate) count: U256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ValidatorConfirmation {
    pub(crate) validator_address: Address,
    pub(crate) tx_hash: B256,
    pub(crate) block_number: u64,
    pub(crate) block_timestamp: NaiveDateTime,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) direction: Option<Direction>,
    // Eth→Gno
    pub(crate) source_request: Option<AnnotatedEvent<UserRequestForAffirmationEvent>>,
    // Gno→Eth
    pub(crate) signature_request: Option<AnnotatedEvent<UserRequestForSignatureEvent>>,
    pub(crate) signatures_collected: Option<AnnotatedEvent<CollectedSignaturesEvent>>,
    // Shared by both directions: a validator confirmation always self-keys
    // to a message of exactly one direction, so there is no collision risk
    // in one map for both `SignedForAffirmation` and `SignedForUserRequest`.
    pub(crate) validator_confirmations: HashMap<Address, ValidatorConfirmation>,
    pub(crate) destination_execution: Option<Completion>,
    /// `receipt.from` of the transaction that emitted the source event
    /// (`UserRequestForAffirmation` or `UserRequestForSignature`). Never
    /// taken from any event field — see the AMB "header sender is not the
    /// source transaction initiator" gotcha, which applies here identically.
    pub(crate) sender_address: Option<Address>,
}

/// `messageHash = keccak256(recipient ‖ value ‖ nonce ‖ foreignBridgeAddr [‖ token])`
/// — `BasicHomeBridge.submitSignature`'s message-blob layout (`Message.sol`),
/// 104 bytes without `token` (Home v6) or 124 with it (Home v7+). Computable
/// at `UserRequestForSignature` time: every component is in the event or in
/// config, so the lookup can be populated proactively rather than waiting for
/// `submitSignature`'s own blob.
pub(crate) fn compute_message_hash(
    recipient: Address,
    value: U256,
    nonce: U256,
    foreign_bridge: Address,
    token: Option<Address>,
) -> B256 {
    let mut preimage = Vec::with_capacity(124);
    preimage.extend_from_slice(recipient.as_slice());
    preimage.extend_from_slice(&value.to_be_bytes::<32>());
    preimage.extend_from_slice(&nonce.to_be_bytes::<32>());
    preimage.extend_from_slice(foreign_bridge.as_slice());
    if let Some(token) = token {
        preimage.extend_from_slice(token.as_slice());
    }
    keccak256(&preimage)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_blob(hex: &str) -> [u8; 32] {
        let bytes = hex::decode(hex.trim_start_matches("0x")).unwrap();
        bytes.try_into().unwrap()
    }

    /// Verified against the official bridge explorer's own encoding.
    #[test]
    fn native_id_blob_round_trips_against_the_verified_explorer_values() {
        assert_eq!(
            native_id_blob(1, U256::from(0x1adf_u64)).unwrap(),
            hex_to_blob("0000000100000000000000000000000000000000000000000000000000001adf")
        );
        assert_eq!(
            native_id_blob(100, U256::from(0x140a_u64)).unwrap(),
            hex_to_blob("000000640000000000000000000000000000000000000000000000000000140a")
        );
    }

    #[test]
    fn native_id_blob_rejects_a_nonce_that_does_not_fit_in_28_bytes() {
        let oversized_nonce = U256::from(1u8) << 225;
        assert!(native_id_blob(1, oversized_nonce).is_err());
    }

    /// Pins the preimage's field order and length against `Message.sol`'s
    /// layout directly (no verified on-chain `messageHash` value exists to
    /// compare against — see the research note's open questions), so this
    /// test exists to catch a field-order or length regression, not to
    /// independently confirm the algorithm.
    #[test]
    fn compute_message_hash_uses_the_104_byte_legacy_layout_without_token() {
        let recipient = Address::repeat_byte(0x11);
        let value = U256::from(1_000u64);
        let nonce = U256::from(0x140a_u64);
        let foreign_bridge = Address::repeat_byte(0x22);

        let mut expected_preimage = Vec::with_capacity(104);
        expected_preimage.extend_from_slice(recipient.as_slice());
        expected_preimage.extend_from_slice(&value.to_be_bytes::<32>());
        expected_preimage.extend_from_slice(&nonce.to_be_bytes::<32>());
        expected_preimage.extend_from_slice(foreign_bridge.as_slice());
        assert_eq!(expected_preimage.len(), 104);

        assert_eq!(
            compute_message_hash(recipient, value, nonce, foreign_bridge, None),
            keccak256(&expected_preimage)
        );
    }

    #[test]
    fn compute_message_hash_uses_the_124_byte_layout_with_token() {
        let recipient = Address::repeat_byte(0x11);
        let value = U256::from(1_000u64);
        let nonce = U256::from(0x140a_u64);
        let foreign_bridge = Address::repeat_byte(0x22);
        let token = Address::repeat_byte(0x33);

        let mut expected_preimage = Vec::with_capacity(124);
        expected_preimage.extend_from_slice(recipient.as_slice());
        expected_preimage.extend_from_slice(&value.to_be_bytes::<32>());
        expected_preimage.extend_from_slice(&nonce.to_be_bytes::<32>());
        expected_preimage.extend_from_slice(foreign_bridge.as_slice());
        expected_preimage.extend_from_slice(token.as_slice());
        assert_eq!(expected_preimage.len(), 124);

        assert_eq!(
            compute_message_hash(recipient, value, nonce, foreign_bridge, Some(token)),
            keccak256(&expected_preimage)
        );
    }

    #[test]
    fn compute_message_hash_differs_between_the_104_and_124_byte_layouts() {
        let recipient = Address::repeat_byte(0x11);
        let value = U256::from(1_000u64);
        let nonce = U256::from(0x140a_u64);
        let foreign_bridge = Address::repeat_byte(0x22);
        let token = Address::repeat_byte(0x33);

        assert_ne!(
            compute_message_hash(recipient, value, nonce, foreign_bridge, None),
            compute_message_hash(recipient, value, nonce, foreign_bridge, Some(token)),
        );
    }
}
