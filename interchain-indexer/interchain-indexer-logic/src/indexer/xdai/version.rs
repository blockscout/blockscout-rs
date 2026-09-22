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

/// One version window of one deployment. The numbers are the proxies' own
/// `EternalStorageProxy.version()` counters, which restart at 1 per
/// deployment — hence the deployment prefix on the testnet variants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum XDaiVersion {
    ForeignV9,
    ForeignV10,
    HomeV6,
    HomeV7,
    SepoliaForeignV2,
    ChiadoHomeV2,
    ChiadoHomeV3,
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
    /// The chain this window's *deployment* lives on, and the first component
    /// of the grammar key. See [`grammar_for`] for why the chain id is part of
    /// the key rather than the side alone.
    ///
    /// This is a deployment discriminator, not a source of chain ids: every
    /// chain id an xDai message writes still comes from `bridges.json` through
    /// `AbiRegistry::chain_ids()`. Nothing here is ever substituted for that.
    pub(crate) chain_id: i64,
    /// First block of this deployment's current identity epoch **on this
    /// side**. Below it the same `topic0`s decoded a *transaction hash* into
    /// the `bytes32` field, not a nonce — silently, with no on-chain signal —
    /// so `AbiRegistry::from_chains` refuses a `started_at_block` beneath it.
    ///
    /// Every window of one side of one deployment declares the same value; it
    /// hangs off the grammar because the floor is a fact about the deployment,
    /// and a deployment is only identifiable once the chain id and version
    /// have selected a window.
    pub(crate) epoch_floor_block: u64,
}

/// Chain ids of the xDai deployments whose protocol constants this module
/// carries. They exist to select *which deployment's* epoch floors and reserve
/// asset apply — see [`XDaiGrammar::chain_id`].
pub(crate) const ETHEREUM_CHAIN_ID: i64 = 1;
pub(crate) const GNOSIS_CHAIN_ID: i64 = 100;
pub(crate) const SEPOLIA_CHAIN_ID: i64 = 11_155_111;
pub(crate) const CHIADO_CHAIN_ID: i64 = 10_200;

/// Ethereum block at which the mainnet Foreign side's nonce epoch begins.
pub(crate) const ETHEREUM_EPOCH_FLOOR_BLOCK: u64 = 22_273_407;
/// Gnosis equivalent of [`ETHEREUM_EPOCH_FLOOR_BLOCK`].
pub(crate) const GNOSIS_EPOCH_FLOOR_BLOCK: u64 = 39_569_937;

/// Sepolia block of the Foreign v1→v2 upgrade (2025-05-02), which is where
/// `UserRequestForAffirmation` gained its `bytes32` nonce. Below it the event
/// is the two-argument form, which carries no identity field at all and has a
/// `topic0` the grammar table deliberately does not contain.
pub(crate) const SEPOLIA_EPOCH_FLOOR_BLOCK: u64 = 8_239_484;
/// First Chiado Home block whose source-event carries a nonce. Below it, the
/// same `topic0`s decoded a *transaction hash* into the `bytes32` field, not a
/// nonce — silently, with no on-chain signal — same as every other deployment's
/// floor (see [`XDaiGrammar::epoch_floor_block`]).
///
/// This is the floor for **both** registered Chiado Home windows
/// (`ChiadoHomeV2` and `ChiadoHomeV3`): the floor is a fact about the
/// deployment's identity epoch, not about which source-event grammar a
/// completion happens to resolve. A hash-keyed `AffirmationCompleted` in this
/// window is not excluded by narrowing the scan — it is resolved by canonical
/// identity: when its source receipt carries a modern source event, the
/// canonical key becomes that event's nonce and the raw hash stays only as
/// observation provenance (see `events.rs::decode_source_evidence` /
/// `SourceEvidence`). Multiple destination executions that resolve to the same
/// canonical identity are handled as multiple-execution anomalies, not by
/// widening or narrowing this floor.
pub(crate) const CHIADO_EPOCH_FLOOR_BLOCK: u64 = 15_562_365;

/// The legacy 104-byte Gno→Eth message layout has no `token` field and
/// hardcodes DAI (`parseMessage`); Home v6 predates the `token` param on
/// `UserRequestForSignature` for the same reason. See
/// [`legacy_home_ethereum_asset`], which is the per-deployment form
/// `consolidation.rs` uses for that fallback.
pub(crate) const DAI: Address = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
pub(crate) const USDS: Address = address!("dC035D45d973E3EC169d2276DDab16f1e407384F");
pub(crate) const LEGACY_DAI_EPOCH_START_BLOCK: u64 = 9_161_003;
pub(crate) const USDS_EPOCH_START_BLOCK: u64 = 23_748_179;

/// The mock 18-decimal `DAI` minted for the Sepolia deployment. It is the
/// Foreign proxy's `erc20token()` at every block probed, from before the first
/// bridge event to `latest`: the testnet has no USDS, no reserve flip and no
/// second asset window.
pub(crate) const SEPOLIA_MOCK_DAI: Address = address!("084Ab2ef1cb3A75EB0fDd81636e9A95D15629c37");
/// Sepolia block at which the Foreign `EternalStorageProxy` was created. The
/// Sepolia analogue of [`LEGACY_DAI_EPOCH_START_BLOCK`]: below it there is no
/// bridge, so there is no asset to name and reconstruction must fail loudly
/// rather than guess.
pub(crate) const SEPOLIA_BRIDGE_CREATION_BLOCK: u64 = 5_339_352;

/// Which ERC-20 the Foreign bridge of `foreign_chain_id` held at
/// `source_block`.
///
/// Two callers, both fallback-only:
///
/// - `consolidation.rs`'s legacy reconstruction path, its original and
///   primary use — messages whose source event is outside the indexed epochs
///   and whose asset therefore cannot come from a decoded log;
/// - `events.rs::resolve_modern_source_asset`, added by
///   `xdai-alias-completion-anomalies`, for a receipt-derived *modern*
///   `UserRequestForAffirmation` whose source block happens to fall below
///   every configured Foreign grammar window. That primary path resolves
///   `source_asset` from the grammar window covering the block (the same
///   window the live stream would select), exactly to keep receipt-derived
///   and stream-derived assembly of the same message in agreement; this
///   function only answers when that window lookup comes back
///   unconfigured — practically unreachable for a genuinely modern event, but
///   the branch must be total rather than panic.
///
/// `foreign_chain_id` is the *Foreign* side's chain id in both directions, not
/// the source chain's: a Gno→Eth message's source block is a Gnosis block and
/// never indexes this table.
pub(crate) fn legacy_ethereum_asset(
    foreign_chain_id: i64,
    direction: Direction,
    source_block: u64,
) -> Result<Address> {
    match foreign_chain_id {
        ETHEREUM_CHAIN_ID => match direction {
            Direction::GnoToEth => Ok(DAI),
            Direction::EthToGno if source_block < LEGACY_DAI_EPOCH_START_BLOCK => bail!(
                "unsupported legacy Ethereum source block {source_block}; DAI epoch starts at {LEGACY_DAI_EPOCH_START_BLOCK}"
            ),
            Direction::EthToGno if source_block < USDS_EPOCH_START_BLOCK => Ok(DAI),
            Direction::EthToGno => Ok(USDS),
        },
        SEPOLIA_CHAIN_ID => match direction {
            Direction::GnoToEth => Ok(SEPOLIA_MOCK_DAI),
            Direction::EthToGno if source_block < SEPOLIA_BRIDGE_CREATION_BLOCK => bail!(
                "unsupported legacy Sepolia source block {source_block}; the xDai Foreign proxy \
                 was created at {SEPOLIA_BRIDGE_CREATION_BLOCK}"
            ),
            Direction::EthToGno => Ok(SEPOLIA_MOCK_DAI),
        },
        _ => bail!(
            "no xDai legacy asset table for Foreign chain {foreign_chain_id}; add the deployment \
             to indexer/xdai/version.rs together with its grammar windows"
        ),
    }
}

/// The Ethereum-side asset the legacy 104-byte `Message.parseMessage`
/// hardcodes for a deployment — the `token_dst_address` of a Gno→Eth message
/// whose `UserRequestForSignature` predates the `token` parameter (Home v6).
///
/// Total on purpose. It is read from `consolidation.rs`, which runs inside the
/// maintenance plan: an `Err` there aborts plan building for the whole bridge
/// on every cycle (see `.memory-bank/rules/error-handling.md`, "Expected Skips
/// Inside a Shared Transaction"), so an unrecognised chain id falls back to the
/// same DAI the pre-multi-deployment code returned unconditionally rather than
/// introducing a new failure mode there.
pub(crate) fn legacy_home_ethereum_asset(foreign_chain_id: i64) -> Address {
    match foreign_chain_id {
        SEPOLIA_CHAIN_ID => SEPOLIA_MOCK_DAI,
        _ => DAI,
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
    chain_id: ETHEREUM_CHAIN_ID,
    epoch_floor_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
};

static FOREIGN_V10_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::ForeignV10,
    side: XDaiSide::Foreign,
    events: FOREIGN_EVENTS,
    canonical_topics: FOREIGN_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: Some(USDS),
    chain_id: ETHEREUM_CHAIN_ID,
    epoch_floor_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
};

static HOME_V6_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::HomeV6,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V6_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: None,
    chain_id: GNOSIS_CHAIN_ID,
    epoch_floor_block: GNOSIS_EPOCH_FLOOR_BLOCK,
};

static HOME_V7_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::HomeV7,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V7_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104Or124,
    source_asset: None,
    chain_id: GNOSIS_CHAIN_ID,
    epoch_floor_block: GNOSIS_EPOCH_FLOOR_BLOCK,
};

/// Sepolia Foreign v2 (`XDaiForeignBridge`, Sourcify full match). The
/// v9-generation contract, not the v10 USDS one: `erc20token()` is the mock
/// DAI and there is no testnet USDS. Every `topic0` in
/// [`FOREIGN_CANONICAL_TOPICS`] was verified against this window's logs, and
/// its `Message.sol` has `isMessageValid(_msg) => _msg.length == 104` with no
/// 124-byte variant.
static SEPOLIA_FOREIGN_V2_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::SepoliaForeignV2,
    side: XDaiSide::Foreign,
    events: FOREIGN_EVENTS,
    canonical_topics: FOREIGN_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: Some(SEPOLIA_MOCK_DAI),
    chain_id: SEPOLIA_CHAIN_ID,
    epoch_floor_block: SEPOLIA_EPOCH_FLOOR_BLOCK,
};

/// Chiado Home v2, the mainnet Home v6 generation: the three-argument
/// `UserRequestForSignature(address,uint256,bytes32)`, with the other four
/// Home `topic0`s unchanged across the whole testnet history. Registered at
/// `started_at_block = 15562365` in `config/xdai/bridges-testnet.json` — the
/// window this deployment's identity epoch actually begins in.
static CHIADO_HOME_V2_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::ChiadoHomeV2,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V6_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: None,
    chain_id: CHIADO_CHAIN_ID,
    epoch_floor_block: CHIADO_EPOCH_FLOOR_BLOCK,
};

/// Chiado Home v3, the mainnet Home v7 generation: the four-argument
/// `UserRequestForSignature(address,uint256,bytes32,address)` verified on
/// chain, with the other four Home `topic0`s unchanged across the whole
/// testnet history.
///
/// v3's `started_at_block` (`20553827` in `bridges-testnet.json`) is a
/// grammar boundary — the block where the Home source-event gained its
/// `token` argument — not an identity boundary. The identity epoch for this
/// deployment begins at [`CHIADO_EPOCH_FLOOR_BLOCK`], shared with
/// `ChiadoHomeV2`; see that constant's doc comment.
///
/// `blob_layout` is the research note's *inference*, not a reading: the v3
/// implementation's source is unverified, but Sepolia Foreign v2 accepts only
/// 104-byte messages, so a 124-byte Home v3 blob could never be executed. The
/// field is documentary today (nothing reads it); revisit if it stops being.
static CHIADO_HOME_V3_GRAMMAR: XDaiGrammar = XDaiGrammar {
    version: XDaiVersion::ChiadoHomeV3,
    side: XDaiSide::Home,
    events: HOME_EVENTS,
    canonical_topics: HOME_V7_CANONICAL_TOPICS,
    identity: IdentityStrategy::Nonce,
    blob_layout: BlobLayout::Len104,
    source_asset: None,
    chain_id: CHIADO_CHAIN_ID,
    epoch_floor_block: CHIADO_EPOCH_FLOOR_BLOCK,
};

/// Selects a version window's grammar from `(chain_id, side, version)`.
///
/// `version` is `bridge_contracts.version` from `bridges.json`, and for xDai
/// it is the proxy's own `EternalStorageProxy.version()` counter, written
/// exactly as the chain reports it. That counter restarts at 1 for each
/// deployment, so the *numbers alone are not a key*: Sepolia's Foreign proxy
/// reports 2 and Chiado's Home proxy reports 3, while Ethereum reports 9/10
/// and Gnosis 6/7.
///
/// The chain id is therefore part of the key, and that is the whole point of
/// it. Keyed on `(side, version)` alone, a mainnet config could select the
/// Sepolia grammar — and with it the 8239484 epoch floor and a mock-DAI
/// `source_asset` — merely by writing `version: 2`, which would index 14M
/// blocks of pre-nonce Ethereum history under the wrong identity semantics and
/// label every deposit with a token that does not exist on Ethereum. With the
/// chain id in the key that config is a hard startup error instead, raised by
/// `AbiRegistry::from_chains` before any indexer runs.
///
/// `getBridgeInterfacesVersion()` cannot be used to discriminate: it returns
/// `6.1.0` on all four proxies.
pub(crate) fn grammar_for(
    chain_id: i64,
    side: XDaiSide,
    version: i16,
) -> Result<&'static XDaiGrammar> {
    match (chain_id, side, version) {
        (ETHEREUM_CHAIN_ID, XDaiSide::Foreign, 9) => Ok(&FOREIGN_V9_GRAMMAR),
        (ETHEREUM_CHAIN_ID, XDaiSide::Foreign, 10) => Ok(&FOREIGN_V10_GRAMMAR),
        (GNOSIS_CHAIN_ID, XDaiSide::Home, 6) => Ok(&HOME_V6_GRAMMAR),
        (GNOSIS_CHAIN_ID, XDaiSide::Home, 7) => Ok(&HOME_V7_GRAMMAR),
        (SEPOLIA_CHAIN_ID, XDaiSide::Foreign, 2) => Ok(&SEPOLIA_FOREIGN_V2_GRAMMAR),
        (CHIADO_CHAIN_ID, XDaiSide::Home, 2) => Ok(&CHIADO_HOME_V2_GRAMMAR),
        (CHIADO_CHAIN_ID, XDaiSide::Home, 3) => Ok(&CHIADO_HOME_V3_GRAMMAR),
        _ => bail!(
            "no xDai grammar registered for chain {chain_id} side {side:?} version {version}. \
             `version` in bridges.json is the proxy's own EternalStorageProxy.version() counter, \
             which restarts per deployment, so it is only meaningful together with the chain id. \
             Registered: Ethereum ({ETHEREUM_CHAIN_ID}) Foreign 9 and 10, Gnosis \
             ({GNOSIS_CHAIN_ID}) Home 6 and 7, Sepolia ({SEPOLIA_CHAIN_ID}) Foreign 2, Chiado \
             ({CHIADO_CHAIN_ID}) Home 2 and 3"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins every mainnet answer to the literal it had before the grammar
    /// table became multi-deployment: same versions, same assets, same floors.
    #[test]
    fn mainnet_grammar_windows_are_unchanged() {
        let foreign_v9 = grammar_for(1, XDaiSide::Foreign, 9).unwrap();
        assert_eq!(foreign_v9.version, XDaiVersion::ForeignV9);
        assert_eq!(foreign_v9.source_asset, Some(DAI));
        assert_eq!(foreign_v9.epoch_floor_block, 22_273_407);

        let foreign_v10 = grammar_for(1, XDaiSide::Foreign, 10).unwrap();
        assert_eq!(foreign_v10.version, XDaiVersion::ForeignV10);
        assert_eq!(foreign_v10.source_asset, Some(USDS));
        assert_eq!(foreign_v10.epoch_floor_block, 22_273_407);

        let home_v6 = grammar_for(100, XDaiSide::Home, 6).unwrap();
        assert_eq!(home_v6.version, XDaiVersion::HomeV6);
        assert_eq!(home_v6.source_asset, None);
        assert_eq!(home_v6.epoch_floor_block, 39_569_937);

        let home_v7 = grammar_for(100, XDaiSide::Home, 7).unwrap();
        assert_eq!(home_v7.version, XDaiVersion::HomeV7);
        assert_eq!(home_v7.source_asset, None);
        assert_eq!(home_v7.epoch_floor_block, 39_569_937);
    }

    #[test]
    fn testnet_grammar_windows_resolve_sepolia_and_chiado_values() {
        let foreign = grammar_for(11_155_111, XDaiSide::Foreign, 2).unwrap();
        assert_eq!(foreign.version, XDaiVersion::SepoliaForeignV2);
        assert_eq!(foreign.source_asset, Some(SEPOLIA_MOCK_DAI));
        assert_eq!(foreign.epoch_floor_block, 8_239_484);
        // The event grammar is the mainnet one verbatim -- every topic0 was
        // verified identical on chain.
        assert!(std::ptr::eq(
            foreign.canonical_topics,
            FOREIGN_V9_GRAMMAR.canonical_topics
        ));

        let home_v2 = grammar_for(10_200, XDaiSide::Home, 2).unwrap();
        assert_eq!(home_v2.version, XDaiVersion::ChiadoHomeV2);
        assert_eq!(home_v2.source_asset, None);
        assert_eq!(home_v2.epoch_floor_block, 15_562_365);
        assert!(std::ptr::eq(
            home_v2.canonical_topics,
            HOME_V6_GRAMMAR.canonical_topics
        ));

        let home = grammar_for(10_200, XDaiSide::Home, 3).unwrap();
        assert_eq!(home.version, XDaiVersion::ChiadoHomeV3);
        assert_eq!(home.source_asset, None);
        assert_eq!(home.epoch_floor_block, 15_562_365);
        assert!(std::ptr::eq(
            home.canonical_topics,
            HOME_V7_GRAMMAR.canonical_topics
        ));
    }

    /// Both Chiado windows declare the same epoch floor: it is a fact about
    /// the deployment's identity epoch, not about which source-event grammar
    /// a given window happens to use.
    #[test]
    fn chiado_epoch_floor_is_shared_by_both_home_windows() {
        assert_eq!(CHIADO_EPOCH_FLOOR_BLOCK, 15_562_365);
        assert_eq!(
            grammar_for(10_200, XDaiSide::Home, 2)
                .unwrap()
                .epoch_floor_block,
            CHIADO_EPOCH_FLOOR_BLOCK
        );
        assert_eq!(
            grammar_for(10_200, XDaiSide::Home, 3)
                .unwrap()
                .epoch_floor_block,
            CHIADO_EPOCH_FLOOR_BLOCK
        );
    }

    /// Mainnet floors are untouched by the Chiado floor change.
    #[test]
    fn mainnet_and_sepolia_floors_are_unchanged_by_the_chiado_floor_move() {
        assert_eq!(ETHEREUM_EPOCH_FLOOR_BLOCK, 22_273_407);
        assert_eq!(GNOSIS_EPOCH_FLOOR_BLOCK, 39_569_937);
        assert_eq!(SEPOLIA_EPOCH_FLOOR_BLOCK, 8_239_484);
    }

    /// The reason the chain id is part of the grammar key: the proxy version
    /// counters restart per deployment, so `version: 2` on Ethereum must not
    /// silently select the Sepolia grammar (and its much lower epoch floor and
    /// mock-DAI asset).
    #[test]
    fn grammar_for_will_not_select_another_deployments_window() {
        assert!(grammar_for(1, XDaiSide::Foreign, 2).is_err());
        assert!(grammar_for(100, XDaiSide::Home, 3).is_err());
        assert!(grammar_for(100, XDaiSide::Home, 2).is_err());
        assert!(grammar_for(11_155_111, XDaiSide::Foreign, 9).is_err());
        assert!(grammar_for(10_200, XDaiSide::Home, 7).is_err());
    }

    #[test]
    fn grammar_for_unknown_chain_side_and_version_returns_error() {
        assert!(grammar_for(1, XDaiSide::Foreign, 11).is_err());
        assert!(grammar_for(100, XDaiSide::Home, 5).is_err());
        assert!(grammar_for(42, XDaiSide::Foreign, 9).is_err());
    }

    #[test]
    fn grammar_for_error_names_the_version_convention() {
        let err = grammar_for(1, XDaiSide::Foreign, 2)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("EternalStorageProxy.version()"),
            "the error must explain what `version` means: {err}"
        );
        assert!(
            err.contains("11155111"),
            "the error must list the registered deployments: {err}"
        );
    }

    #[test]
    fn legacy_asset_uses_only_ethereum_blocks_for_eth_to_gno() {
        assert!(legacy_ethereum_asset(1, Direction::EthToGno, 9_161_002).is_err());
        assert_eq!(
            legacy_ethereum_asset(1, Direction::EthToGno, 9_161_003).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(1, Direction::EthToGno, 23_748_178).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(1, Direction::EthToGno, 23_748_179).unwrap(),
            USDS
        );
    }

    #[test]
    fn legacy_gno_to_eth_always_resolves_dai() {
        assert_eq!(
            legacy_ethereum_asset(1, Direction::GnoToEth, 39_557_691).unwrap(),
            DAI
        );
        assert_eq!(
            legacy_ethereum_asset(1, Direction::GnoToEth, u64::MAX).unwrap(),
            DAI
        );
    }

    /// Sepolia has one asset for its whole life and no cutover, so the only
    /// boundary is the proxy's own creation block.
    #[test]
    fn legacy_asset_resolves_the_sepolia_mock_dai_in_both_directions() {
        assert!(legacy_ethereum_asset(11_155_111, Direction::EthToGno, 5_339_351).is_err());
        assert_eq!(
            legacy_ethereum_asset(11_155_111, Direction::EthToGno, 5_339_352).unwrap(),
            SEPOLIA_MOCK_DAI
        );
        assert_eq!(
            legacy_ethereum_asset(11_155_111, Direction::EthToGno, 23_748_179).unwrap(),
            SEPOLIA_MOCK_DAI,
            "the mainnet USDS cutover must not leak into the Sepolia table"
        );
        assert_eq!(
            legacy_ethereum_asset(11_155_111, Direction::GnoToEth, u64::MAX).unwrap(),
            SEPOLIA_MOCK_DAI
        );
    }

    #[test]
    fn legacy_asset_rejects_an_unregistered_foreign_chain() {
        assert!(legacy_ethereum_asset(42, Direction::EthToGno, 23_748_179).is_err());
        assert!(legacy_ethereum_asset(42, Direction::GnoToEth, 1).is_err());
    }

    #[test]
    fn legacy_home_asset_is_per_deployment_and_total() {
        assert_eq!(legacy_home_ethereum_asset(1), DAI);
        assert_eq!(legacy_home_ethereum_asset(11_155_111), SEPOLIA_MOCK_DAI);
        // Unreachable in practice (the registry validates chain ids first);
        // pinned because it must not become a maintenance-path failure.
        assert_eq!(legacy_home_ethereum_asset(42), DAI);
    }
}
