use alloy::primitives::{Address, address};
use anyhow::{Result, bail};

use super::types::Direction;

/// Vocabulary follows the underlying `tokenbridge-contracts`: **Home =
/// Gnosis**, **Foreign = Ethereum** — counter-intuitive, and the source of
/// the `ForeignToHome` / `HomeToForeign` naming seen in `amb/types.rs`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum XDaiSide {
    Foreign,
    Home,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XDaiVersion {
    ForeignV9,
    ForeignV10,
    HomeV6,
    HomeV7,
}

/// Source-event identity derivation for the configured scan epochs. Some
/// destination events in those same windows still carry a legacy source
/// transaction hash and are classified at event level.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdentityStrategy {
    Nonce,
}

/// Length of the `Message.sol` blob `submitSignature` accepts on the Home
/// side. `Len104` is the legacy form (no `token`); `Len104Or124` additionally
/// permits the 124-byte form carrying `tokenAddress` (Home v7+). Not
/// meaningful on the Foreign side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BlobLayout {
    Len104,
    Len104Or124,
}

/// One version window's protocol grammar: which events it declares, the
/// canonical signature each must hash to (see `abi::assert_canonical_topics`
/// — this is what tells an xDai ABI apart from an AMB one sharing the same
/// event names), how identity is derived, the message-blob shape, and —
/// Foreign only — which asset the bridge held during this window.
#[derive(Clone, Copy, Debug)]
pub(crate) struct XDaiGrammar {
    pub(crate) version: XDaiVersion,
    pub(crate) side: XDaiSide,
    pub(crate) events: &'static [&'static str],
    /// `(event name, canonical Solidity signature)`. The expected `topic0` is
    /// `keccak256(signature)`, computed the same way `Event::selector()`
    /// derives it from a real ABI event.
    pub(crate) canonical_topics: &'static [(&'static str, &'static str)],
    #[allow(dead_code)]
    pub(crate) identity: IdentityStrategy,
    #[allow(dead_code)]
    pub(crate) blob_layout: BlobLayout,
    /// Ethereum-side asset held during this window. `Some` for every Foreign
    /// window; `None` for Home (the Home side is always native xDAI, which
    /// never comes from this table).
    pub(crate) source_asset: Option<Address>,
}

/// Ethereum block at which the current epoch's Foreign grammar begins.
/// Below this, the same `topic0`s decoded a *transaction hash* into the
/// `bytes32` field, not a nonce — silently, with no on-chain signal.
pub(crate) const FOREIGN_EPOCH_FLOOR_BLOCK: u64 = 22_273_407;
/// Gnosis equivalent of [`FOREIGN_EPOCH_FLOOR_BLOCK`].
pub(crate) const HOME_EPOCH_FLOOR_BLOCK: u64 = 39_569_937;

/// The legacy 104-byte Gno→Eth message layout has no `token` field and
/// hardcodes DAI (`parseMessage`); Home v6 predates the `token` param on
/// `UserRequestForSignature` for the same reason. Exposed for
/// `consolidation.rs`'s transfer construction, which needs the same
/// fallback.
pub(crate) const DAI: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
pub(crate) const USDS: Address = address!("dC035D45d973E3EC169d2276DDab16f1e407384F");
pub(crate) const LEGACY_DAI_EPOCH_START_BLOCK: u64 = 9_161_003;
pub(crate) const USDS_EPOCH_START_BLOCK: u64 = 23_748_179;

pub(crate) fn legacy_ethereum_asset(direction: Direction, source_block: u64) -> Result<Address> {
    match direction {
        Direction::GnoToEth => Ok(DAI),
        Direction::EthToGno if source_block < LEGACY_DAI_EPOCH_START_BLOCK => bail!(
            "unsupported legacy Ethereum source block {source_block}; DAI epoch starts at {LEGACY_DAI_EPOCH_START_BLOCK}"
        ),
        Direction::EthToGno if source_block < USDS_EPOCH_START_BLOCK => Ok(DAI),
        Direction::EthToGno => Ok(USDS),
    }
}

pub(crate) static FOREIGN_EVENTS: &[&str] = &["UserRequestForAffirmation", "RelayedMessage"];
pub(crate) static HOME_EVENTS: &[&str] = &[
    "UserRequestForSignature",
    "AffirmationCompleted",
    "SignedForAffirmation",
    "SignedForUserRequest",
    "CollectedSignatures",
];

static FOREIGN_CANONICAL_TOPICS: &[(&str, &str)] = &[
    (
        "UserRequestForAffirmation",
        "UserRequestForAffirmation(address,uint256,bytes32)",
    ),
    ("RelayedMessage", "RelayedMessage(address,uint256,bytes32)"),
];

static HOME_V6_CANONICAL_TOPICS: &[(&str, &str)] = &[
    (
        "UserRequestForSignature",
        "UserRequestForSignature(address,uint256,bytes32)",
    ),
    (
        "AffirmationCompleted",
        "AffirmationCompleted(address,uint256,bytes32)",
    ),
    (
        "SignedForAffirmation",
        "SignedForAffirmation(address,bytes32)",
    ),
    (
        "SignedForUserRequest",
        "SignedForUserRequest(address,bytes32)",
    ),
    (
        "CollectedSignatures",
        "CollectedSignatures(address,bytes32,uint256)",
    ),
];

static HOME_V7_CANONICAL_TOPICS: &[(&str, &str)] = &[
    (
        "UserRequestForSignature",
        "UserRequestForSignature(address,uint256,bytes32,address)",
    ),
    (
        "AffirmationCompleted",
        "AffirmationCompleted(address,uint256,bytes32)",
    ),
    (
        "SignedForAffirmation",
        "SignedForAffirmation(address,bytes32)",
    ),
    (
        "SignedForUserRequest",
        "SignedForUserRequest(address,bytes32)",
    ),
    (
        "CollectedSignatures",
        "CollectedSignatures(address,bytes32,uint256)",
    ),
];

static FOREIGN_V9_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::ForeignV9,
    side: XDaiSide::Foreign,
    events: FOREIGN_EVENTS,
    canonical_topics: FOREIGN_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: Some(DAI),
};

static FOREIGN_V10_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::ForeignV10,
    side: XDaiSide::Foreign,
    events: FOREIGN_EVENTS,
    canonical_topics: FOREIGN_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: Some(USDS),
};

static HOME_V6_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::HomeV6,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V6_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: None,
};

static HOME_V7_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::HomeV7,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V7_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104Or124,
    source_asset: None,
};

pub(crate) fn grammar_for(side: XDaiSide, version: i16) -> Result<&'static XDaiGrammar> {
    match (side, version) {
        (XDaiSide::Foreign, 9) => Ok(&FOREIGN_V9_GRAMMAR),
        (XDaiSide::Foreign, 10) => Ok(&FOREIGN_V10_GRAMMAR),
        (XDaiSide::Home, 6) => Ok(&HOME_V6_GRAMMAR),
        (XDaiSide::Home, 7) => Ok(&HOME_V7_GRAMMAR),
        _ => bail!("no xDai grammar registered for side {side:?} version {version}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grammar_for_returns_the_right_grammar_per_side_and_version() {
        assert_eq!(
            grammar_for(XDaiSide::Foreign, 9).unwrap().version,
            XDaiVersion::ForeignV9
        );
        assert_eq!(
            grammar_for(XDaiSide::Foreign, 9).unwrap().source_asset,
            Some(DAI)
        );
        assert_eq!(
            grammar_for(XDaiSide::Foreign, 10).unwrap().version,
            XDaiVersion::ForeignV10
        );
        assert_eq!(
            grammar_for(XDaiSide::Foreign, 10).unwrap().source_asset,
            Some(USDS)
        );
        assert_eq!(
            grammar_for(XDaiSide::Home, 6).unwrap().version,
            XDaiVersion::HomeV6
        );
        assert_eq!(
            grammar_for(XDaiSide::Home, 7).unwrap().version,
            XDaiVersion::HomeV7
        );
    }

    #[test]
    fn grammar_for_unknown_side_and_version_returns_error() {
        assert!(grammar_for(XDaiSide::Foreign, 11).is_err());
        assert!(grammar_for(XDaiSide::Home, 5).is_err());
    }

    #[test]
    fn legacy_asset_uses_only_ethereum_blocks_for_eth_to_gno() {
        assert!(legacy_ethereum_asset(Direction::EthToGno, 9_161_002).is_err());
        assert_eq!(
            legacy_ethereum_asset(Direction::EthToGno, 9_161_003).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(Direction::EthToGno, 23_748_178).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(Direction::EthToGno, 23_748_179).unwrap(),
            USDS
        );
    }

    #[test]
    fn legacy_gno_to_eth_always_resolves_dai() {
        assert_eq!(
            legacy_ethereum_asset(Direction::GnoToEth, 39_557_691).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(Direction::GnoToEth, u64::MAX).unwrap(),
            DAI
        );
    }
}
