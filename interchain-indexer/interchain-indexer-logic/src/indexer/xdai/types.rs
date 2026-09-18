use std::collections::HashMap;

use alloy::primitives::{Address, B256, U256, keccak256};
use anyhow::{Context, Result, ensure};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use crate::message_buffer::Key;

use super::version::XDaiSide;

/// Sentinel written for the Gnosis leg of a transfer, which is always native
/// xDAI and therefore has no token contract to record. A named constant, not
/// a literal at each call site: if another bridge later writes a native leg
/// on chain 100, both must use this exact value or they form two disjoint
/// `stats_assets` rows for the same coin. See the gotcha in
/// `.memory-bank/gotchas.md` for the full rationale, including why the
/// matching `tokens` row must be seeded.
pub(crate) const NATIVE_SENTINEL: Address = Address::ZERO;

/// The nonce is a **per-contract** monotonic counter, so Eth→Gno and
/// Gno→Eth have independent, overlapping ranges. The initiator's chain id is
/// what makes identity globally unique — never key on the bare nonce.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Direction {
    EthToGno,
    GnoToEth,
}

impl Direction {
    /// A direction names *sides*, never chain ids: `Eth` is whichever chain
    /// is configured as [`XDaiSide::Foreign`], `Gno` whichever is
    /// [`XDaiSide::Home`]. The ids themselves come from `bridges.json` via
    /// [`ChainIds`] — hardcoding 1/100 here made every non-mainnet pair write
    /// chain ids that do not exist in the `chains` table.
    pub(crate) fn initiator_side(self) -> XDaiSide {
        match self {
            Direction::EthToGno => XDaiSide::Foreign,
            Direction::GnoToEth => XDaiSide::Home,
        }
    }

    pub(crate) fn destination_side(self) -> XDaiSide {
        match self {
            Direction::EthToGno => XDaiSide::Home,
            Direction::GnoToEth => XDaiSide::Foreign,
        }
    }
}

/// This bridge's two configured endpoints, resolved once from
/// `abi::AbiRegistry`'s side map (itself built from `bridges.json`).
///
/// AMB carries the equivalent per event on `AnnotatedEvent`; xDai carries it
/// per *message* instead, because a direction — not a single event — is what
/// picks which end is the initiator, and `Consolidate::consolidate` sees only
/// the buffered message. Kept side-keyed rather than as a resolved
/// `(src, dst)` pair so the value stays independent of the direction, which
/// different handlers may set at different times.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChainIds {
    pub(crate) foreign: i64,
    pub(crate) home: i64,
}

impl ChainIds {
    pub(crate) fn for_side(self, side: XDaiSide) -> i64 {
        match side {
            XDaiSide::Foreign => self.foreign,
            XDaiSide::Home => self.home,
        }
    }

    pub(crate) fn initiator(self, direction: Direction) -> i64 {
        self.for_side(direction.initiator_side())
    }

    pub(crate) fn destination(self, direction: Direction) -> i64 {
        self.for_side(direction.destination_side())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MessageIdentity {
    Nonce(U256),
    SourceTransactionHash(B256),
}

impl MessageIdentity {
    pub(crate) fn source(value: U256) -> Result<Self> {
        ensure!(
            value <= U256::from(u64::MAX),
            "xDai source nonce {value} exceeds u64::MAX"
        );
        Ok(Self::Nonce(value))
    }

    pub(crate) fn destination(value: U256) -> Self {
        if value <= U256::from(u64::MAX) {
            Self::Nonce(value)
        } else {
            Self::SourceTransactionHash(B256::from(value.to_be_bytes::<32>()))
        }
    }

    pub(crate) fn native_id(self, direction: Direction, chain_ids: ChainIds) -> Result<[u8; 32]> {
        match self {
            Self::Nonce(nonce) => {
                ensure!(
                    nonce <= U256::from(u64::MAX),
                    "xDai nonce {nonce} exceeds u64::MAX"
                );
                native_id_blob(chain_ids.initiator(direction), nonce)
            }
            Self::SourceTransactionHash(hash) => Ok(hash.0),
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
/// than silently truncate. The same applies to the chain id: the encoding has
/// exactly 4 bytes for it, which every realistic pair fits (Sepolia's
/// 11155111 and Chiado's 10200 both sit far below `u32::MAX`), but a chain id
/// beyond that range would otherwise be truncated into a *different* valid
/// blob and collide with another chain's identities.
pub(crate) fn native_id_blob(initiator_chain_id: i64, nonce: U256) -> Result<[u8; 32]> {
    let nonce_bytes = nonce.to_be_bytes::<32>();
    ensure!(
        nonce_bytes[0..4] == [0u8; 4],
        "xDai nonce {nonce} does not fit in the 28-byte native_id encoding"
    );
    let initiator_chain_id = u32::try_from(initiator_chain_id).with_context(|| {
        format!(
            "xDai initiator chain id {initiator_chain_id} does not fit in the 4-byte native_id \
             encoding"
        )
    })?;

    let mut blob = [0u8; 32];
    blob[0..4].copy_from_slice(&initiator_chain_id.to_be_bytes());
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AnnotatedEvent<T> {
    pub(crate) event: T,
    pub(crate) transaction_hash: B256,
    pub(crate) block_number: i64,
    pub(crate) block_timestamp: NaiveDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
/// `(recipient, value)` beyond the identity, which is stored once on the
/// buffered message.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

    pub(crate) fn direction(&self) -> Direction {
        match self {
            Self::Affirmation(_) => Direction::EthToGno,
            Self::Relayed(_) => Direction::GnoToEth,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UserRequestForSignatureEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
    pub(crate) nonce: U256,
    /// Destination-side asset, explicit only from Home v7. `None` below v7
    /// means DAI — the legacy 104-byte message layout hardcodes it.
    pub(crate) token: Option<Address>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CollectedSignaturesEvent {
    pub(crate) authority_responsible_for_relay: Address,
    pub(crate) message_hash: B256,
    /// `requiredSignatures()`, **not** the number actually collected — do
    /// not treat it as a tally.
    pub(crate) count: U256,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ValidatorConfirmation {
    pub(crate) validator_address: Address,
    pub(crate) tx_hash: B256,
    pub(crate) block_number: u64,
    pub(crate) block_timestamp: NaiveDateTime,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LegacySourceEvent {
    pub(crate) recipient: Address,
    pub(crate) value: U256,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReconstructedSource {
    pub(crate) transaction_hash: B256,
    pub(crate) block_number: u64,
    pub(crate) block_timestamp: NaiveDateTime,
    pub(crate) sender_address: Address,
    pub(crate) ethereum_asset: Address,
    pub(crate) legacy_source_event: Option<LegacySourceEvent>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Message {
    pub(crate) identity: Option<MessageIdentity>,
    pub(crate) direction: Option<Direction>,
    /// The configured Foreign/Home chain ids, stamped by every handler that
    /// sets `direction` — so `chain_ids.is_some()` whenever `direction.is_some()`,
    /// which is exactly what consolidation needs.
    ///
    /// `serde(default)` because the buffer persists this struct as JSON in
    /// `pending_messages.payload`: a payload written before the field existed
    /// must still deserialize. Such an entry revives as `direction: Some(..)`
    /// with `chain_ids: None` and can never consolidate — the ids are not
    /// recoverable from the payload — so `consolidation.rs` treats it as
    /// **not consolidatable** (warn + `XDAI_MESSAGES_MISSING_CHAIN_IDS`),
    /// never as an error. An error there propagates out of
    /// `maintenance.rs::classify_item` into `plan_maintenance` before the
    /// maintenance transaction opens and stalls the whole bridge's flush,
    /// stats projection and cursor persistence permanently; see the comment
    /// at that skip site.
    #[serde(default)]
    pub(crate) chain_ids: Option<ChainIds>,
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
    pub(crate) reconstructed_source: Option<ReconstructedSource>,
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

    /// Ethereum/Gnosis, the pair the verified explorer values below come from.
    const MAINNET: ChainIds = ChainIds {
        foreign: 1,
        home: 100,
    };

    /// Sepolia/Chiado: a Foreign/Home pair that is neither 1 nor 100, so a
    /// literal creeping back into the encoding cannot pass.
    const TESTNET: ChainIds = ChainIds {
        foreign: 11_155_111,
        home: 10_200,
    };

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

    /// B1: `native_id`'s first 4 bytes are the *configured* initiator chain
    /// id, so a Sepolia/Chiado pair encodes 11155111/10200 — not 1/100.
    #[test]
    fn native_id_uses_the_configured_initiator_chain_id_on_a_non_mainnet_pair() {
        let identity = MessageIdentity::Nonce(U256::from(0x1adf_u64));

        assert_eq!(
            identity.native_id(Direction::EthToGno, TESTNET).unwrap(),
            hex_to_blob("00aa36a700000000000000000000000000000000000000000000000000001adf"),
        );
        assert_eq!(
            identity.native_id(Direction::GnoToEth, TESTNET).unwrap(),
            hex_to_blob("000027d800000000000000000000000000000000000000000000000000001adf"),
        );
    }

    #[test]
    fn chain_ids_resolve_each_direction_to_its_configured_endpoints() {
        assert_eq!(TESTNET.initiator(Direction::EthToGno), 11_155_111);
        assert_eq!(TESTNET.destination(Direction::EthToGno), 10_200);
        assert_eq!(TESTNET.initiator(Direction::GnoToEth), 10_200);
        assert_eq!(TESTNET.destination(Direction::GnoToEth), 11_155_111);
    }

    /// Sepolia's 11155111 fits the 4-byte field with room to spare; the guard
    /// exists for a chain id that does not, which would otherwise be truncated
    /// into another chain's valid blob.
    #[test]
    fn native_id_blob_rejects_a_chain_id_that_does_not_fit_in_four_bytes() {
        assert!(native_id_blob(11_155_111, U256::from(1u8)).is_ok());
        assert!(native_id_blob(i64::from(u32::MAX), U256::from(1u8)).is_ok());
        assert!(native_id_blob(i64::from(u32::MAX) + 1, U256::from(1u8)).is_err());
    }

    #[test]
    fn message_identity_classifies_source_and_destination_boundaries() {
        for nonce in [U256::ZERO, U256::from(u64::MAX)] {
            assert_eq!(
                MessageIdentity::source(nonce).unwrap(),
                MessageIdentity::Nonce(nonce)
            );
            assert_eq!(
                MessageIdentity::destination(nonce),
                MessageIdentity::Nonce(nonce)
            );
        }

        let two_to_64 = U256::from(u64::MAX) + U256::from(1u8);
        assert!(MessageIdentity::source(two_to_64).is_err());
        assert_eq!(
            MessageIdentity::destination(two_to_64),
            MessageIdentity::SourceTransactionHash(B256::from(two_to_64.to_be_bytes::<32>()))
        );
        assert_eq!(
            MessageIdentity::destination(U256::MAX),
            MessageIdentity::SourceTransactionHash(B256::from(U256::MAX.to_be_bytes::<32>()))
        );
    }

    #[test]
    fn hash_identity_native_id_preserves_all_leading_zero_bytes() {
        for bytes in [
            hex_to_blob("0000000100000001000000000000000000000000000000000000000000000000"),
            hex_to_blob("0000000000000001000000000000000000000000000000000000000000000000"),
        ] {
            let identity = MessageIdentity::destination(U256::from_be_bytes(bytes));
            assert_eq!(
                identity,
                MessageIdentity::SourceTransactionHash(B256::from(bytes))
            );
            assert_eq!(
                identity.native_id(Direction::EthToGno, MAINNET).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn manually_constructed_oversized_nonce_is_rejected() {
        let identity = MessageIdentity::Nonce(U256::from(u64::MAX) + U256::from(1u8));
        assert!(identity.native_id(Direction::EthToGno, MAINNET).is_err());
    }

    #[test]
    fn all_incident_observations_classify_as_source_transaction_hashes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../tests/fixtures/xdai/legacy/identity_observations.json"
        ))
        .unwrap();
        let observations = fixture["observations"].as_array().unwrap();
        assert_eq!(observations.len(), 352);

        for observation in observations {
            let hash: B256 = observation["identity"].as_str().unwrap().parse().unwrap();
            let value = U256::from_be_slice(hash.as_slice());
            assert_eq!(
                MessageIdentity::destination(value),
                MessageIdentity::SourceTransactionHash(hash)
            );
            assert!(MessageIdentity::source(value).is_err());
        }
    }

    #[test]
    fn pending_message_state_round_trips_with_reconstructed_source() {
        let source_hash = B256::repeat_byte(0x35);
        let message = Message {
            identity: Some(MessageIdentity::SourceTransactionHash(source_hash)),
            direction: Some(Direction::GnoToEth),
            reconstructed_source: Some(ReconstructedSource {
                transaction_hash: source_hash,
                block_number: 39_557_691,
                block_timestamp: chrono::DateTime::from_timestamp(1_744_646_380, 0)
                    .unwrap()
                    .naive_utc(),
                sender_address: Address::repeat_byte(1),
                ethereum_asset: Address::repeat_byte(2),
                legacy_source_event: Some(LegacySourceEvent {
                    recipient: Address::repeat_byte(3),
                    value: U256::from(49_240u64),
                }),
            }),
            ..Default::default()
        };
        let encoded = serde_json::to_value(&message).unwrap();
        let decoded: Message = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.identity, message.identity);
        assert_eq!(decoded.direction, message.direction);
        assert_eq!(decoded.reconstructed_source, message.reconstructed_source);
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
