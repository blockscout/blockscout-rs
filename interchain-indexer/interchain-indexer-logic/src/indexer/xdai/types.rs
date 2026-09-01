use std::collections::HashMap;

use alloy::primitives::{Address, B256, U256, keccak256};
use anyhow::{Context, Result, ensure};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::message_buffer::Key;

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct AffirmationCompletedEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
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
    pub(crate) source_request: Option<AnnotatedEvent<UserRequestForAffirmationEvent>>,
    pub(crate) validator_confirmations: HashMap<Address, ValidatorConfirmation>,
    pub(crate) destination_execution: Option<AnnotatedEvent<AffirmationCompletedEvent>>,
    /// `receipt.from` of the transaction that emitted `source_request`. Never
    /// taken from any event field — see the AMB "header sender is not the
    /// source transaction initiator" gotcha, which applies here identically.
    pub(crate) sender_address: Option<Address>,
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
}
