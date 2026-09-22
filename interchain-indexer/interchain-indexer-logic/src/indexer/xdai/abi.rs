use std::collections::HashMap;

use alloy::{
    json_abi::JsonAbi,
    primitives::{Address, B256, keccak256},
    rpc::types::Filter,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

use crate::indexer::evm::abi_registry;

use super::{
    indexer::XDaiChainConfig,
    types::{ChainIds, Direction},
    version::{
        FOREIGN_EVENTS, HOME_EVENTS, USDS_EPOCH_START_BLOCK, XDaiGrammar, XDaiSide, grammar_for,
        legacy_ethereum_asset,
    },
};

/// xDai has exactly one contract kind per chain (unlike AMB's proxy +
/// mediator), so this only ever carries the inferred side plus the matched
/// version. `version` matters beyond bookkeeping: it is the key into
/// `grammar_for`, which is how a decoded log's `source_asset` (DAI vs USDS,
/// Foreign v9 vs v10) is recovered without a second registry lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContractKind {
    pub(crate) side: XDaiSide,
    pub(crate) version: i16,
}

pub(crate) type LogResolution<'a> = abi_registry::LogResolution<'a, ContractKind>;

/// Thin xDai-specific wrapper around the protocol-agnostic
/// [`abi_registry::AbiRegistry`], mirroring `amb::abi::AbiRegistry`'s shape.
/// The version-window resolution is shared with AMB (ADR-006); the
/// Home/Foreign side map is not — xDai's fixed two-chain topology needs its
/// own copy, not AMB's.
#[derive(Clone, Debug, Default)]
pub(crate) struct AbiRegistry {
    inner: abi_registry::AbiRegistry<ContractKind>,
    chain_by_side: HashMap<XDaiSide, i64>,
}

impl AbiRegistry {
    pub(crate) fn from_chains(chains: &[XDaiChainConfig]) -> Result<Self> {
        let mut registry = Self::default();

        for chain in chains {
            ensure!(
                !chain.contracts.is_empty(),
                "xDai chain {} has no configured contract",
                chain.chain_id
            );

            // The side is a property of the chain, not of a version: an
            // upgrade cannot turn a Home contract into a Foreign one. Every
            // configured version must agree, or the config describes two
            // different bridges under one chain id.
            let mut chain_side: Option<XDaiSide> = None;

            for contract in &chain.contracts {
                let side = side_for_abi(chain.chain_id, contract.address, contract.abi.as_ref())?;
                match chain_side {
                    None => chain_side = Some(side),
                    Some(existing) => ensure!(
                        existing == side,
                        "xDai chain {} has contract versions on different sides ({existing:?} and {side:?})",
                        chain.chain_id
                    ),
                }

                // Keyed on the chain id as well as the side: the proxy version
                // counters restart per deployment, so `(side, version)` alone
                // would let one deployment's config select another's floors and
                // reserve asset. See `version::grammar_for`.
                let grammar = grammar_for(chain.chain_id, side, contract.version)?;
                debug_assert_eq!(
                    grammar.side, side,
                    "grammar_for returned a grammar registered under the wrong side"
                );
                debug_assert_eq!(
                    grammar.chain_id, chain.chain_id,
                    "grammar_for returned a grammar registered under the wrong chain"
                );

                // The event-name partition is byte-for-byte identical to
                // AMB's, so side inference alone cannot tell the two
                // protocols apart -- only the canonical topic0 can. This is
                // what stops an AMB ABI from being accepted as xDai (or vice
                // versa) under a plausible-looking config.
                assert_canonical_topics(
                    chain.chain_id,
                    contract.address,
                    contract.abi.as_ref(),
                    grammar,
                )?;

                // Below the floor, these same topic0s decoded a *transaction
                // hash* into the bytes32 field, not a nonce -- silently, with
                // no on-chain signal. See ADR-006 / the protocol primer. The
                // floor is per deployment, so it comes off the grammar the
                // (chain, side, version) key just selected rather than from a
                // module-level constant.
                let floor = grammar.epoch_floor_block;
                ensure!(
                    contract.started_at_block >= floor,
                    "xDai chain {} contract {} version {} started_at_block {} is below the \
                     {side:?} epoch floor {floor}: below the floor the bytes32 fields in these \
                     events mean transaction hash, not nonce, under an unchanged topic0, so \
                     identities would be silently wrong",
                    chain.chain_id,
                    contract.address,
                    contract.version,
                    contract.started_at_block,
                );

                assert_epoch_boundaries_agree(chain.chain_id, contract.started_at_block, grammar)?;

                registry.inner.insert_contract(
                    chain.chain_id,
                    contract.address,
                    contract.started_at_block,
                    ContractKind {
                        side,
                        version: contract.version,
                    },
                    contract.abi.as_ref(),
                    grammar.events,
                )?;

                tracing::debug!(
                    chain_id = chain.chain_id,
                    address = %contract.address,
                    grammar_version = ?grammar.version,
                    started_at_block = contract.started_at_block,
                    side = ?side,
                    "registered xDai contract version"
                );
            }

            let side = chain_side.expect("non-empty contracts yields a side");
            ensure!(
                registry
                    .chain_by_side
                    .insert(side, chain.chain_id)
                    .is_none(),
                "xDai bridge config has multiple {side:?} chains"
            );
        }

        Ok(registry)
    }

    pub(crate) fn chain_id_for_side(&self, side: XDaiSide) -> Result<i64> {
        self.chain_by_side
            .get(&side)
            .copied()
            .with_context(|| format!("xDai bridge config missing {side:?} chain"))
    }

    pub(crate) fn counterpart_chain_id(&self, side: XDaiSide) -> Result<i64> {
        let counterpart = match side {
            XDaiSide::Foreign => XDaiSide::Home,
            XDaiSide::Home => XDaiSide::Foreign,
        };
        self.chain_id_for_side(counterpart)
    }

    /// Both configured endpoints as one value, for stamping onto a buffered
    /// message. Every chain id an xDai message writes — `src_chain_id`,
    /// `dst_chain_id`, both transfer legs and the first 4 bytes of
    /// `native_id` — is resolved through this, never from a literal.
    pub(crate) fn chain_ids(&self) -> Result<ChainIds> {
        let foreign = self.chain_id_for_side(XDaiSide::Foreign)?;
        let home = self.counterpart_chain_id(XDaiSide::Foreign)?;
        Ok(ChainIds { foreign, home })
    }

    /// The Foreign proxy's own address — the `foreignBridgeAddr` component of
    /// `messageHash = keccak256(recipient ‖ value ‖ nonce ‖ foreignBridgeAddr [‖ token])`.
    /// Every configured Foreign version shares one address (a proxy is
    /// upgraded behind the same address, like every other xDai/AMB
    /// contract); this fails loudly if the config somehow disagrees rather
    /// than silently picking one.
    pub(crate) fn foreign_proxy_address(&self) -> Result<Address> {
        let foreign_chain_id = self.chain_id_for_side(XDaiSide::Foreign)?;
        let mut addresses = self
            .inner
            .contracts
            .keys()
            .filter(|(chain_id, _)| *chain_id == foreign_chain_id)
            .map(|(_, address)| *address);
        let first = addresses
            .next()
            .context("no xDai Foreign contract configured")?;
        ensure!(
            addresses.all(|address| address == first),
            "xDai Foreign side has more than one configured proxy address"
        );
        Ok(first)
    }

    // `side_for_chain` and `event_for_log` are not needed. xDai's "direction
    // is derived, never looked up" design still holds for the *side*: the
    // event itself says which direction a message travels, so no handler ever
    // asks which side a chain is on, and no phase looks up a second event in
    // the same receipt at runtime. It does **not** extend to the chain ids
    // behind a direction: those are config, and `chain_ids` above is the only
    // way to obtain them.

    pub(crate) fn resolve_log(
        &self,
        chain_id: i64,
        address: Address,
        topic: &B256,
        block_number: u64,
    ) -> LogResolution<'_> {
        self.inner
            .resolve_log(chain_id, address, topic, block_number)
    }

    pub(crate) fn filter_for_chain(&self, chain_id: i64) -> Result<Filter> {
        self.inner.filter_for_chain(chain_id)
    }
}

/// Infers `XDaiSide` from the ABI's declared event-name set, exactly as
/// `amb::abi::amb_side_for_abi` does for AMB. Deliberately **not** backed by
/// a config `kind`: `BridgeContractConfig.kind` is optional and unused by
/// Avalanche; AMB needed it only because it puts two different contract
/// *types* on one side, and xDai has one contract per chain.
fn side_for_abi(chain_id: i64, address: Address, abi_value: Option<&Value>) -> Result<XDaiSide> {
    let abi_value = abi_value.with_context(|| {
        format!("missing ABI for xDai contract row chain_id={chain_id} address={address}")
    })?;
    let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
        format!("invalid ABI for xDai contract row chain_id={chain_id} address={address}")
    })?;

    let has_foreign_events = FOREIGN_EVENTS
        .iter()
        .all(|event_name| abi.events.contains_key(*event_name));
    let has_home_events = HOME_EVENTS
        .iter()
        .all(|event_name| abi.events.contains_key(*event_name));

    match (has_foreign_events, has_home_events) {
        (true, false) => Ok(XDaiSide::Foreign),
        (false, true) => Ok(XDaiSide::Home),
        (true, true) => bail!(
            "xDai ABI for chain_id={chain_id} address={address} contains both Home and Foreign event sets"
        ),
        (false, false) => bail!(
            "xDai ABI for chain_id={chain_id} address={address} does not match a Home or Foreign event set"
        ),
    }
}

/// Asserts that the two independent answers to "which asset did the Ethereum
/// side hold at block N" agree for every configured Foreign window.
///
/// The Ethereum-side asset is resolved two ways, and only one of them is
/// config-driven: the current-epoch path reads `XDaiGrammar::source_asset`
/// for the version window `started_at_block` selects, while the legacy
/// reconstruction path in `version::legacy_ethereum_asset` splits on the
/// hardcoded [`USDS_EPOCH_START_BLOCK`]. They line up today only because
/// `bridges.json` happens to start Foreign v10 at exactly that block; an
/// operator moving `started_at_block` (an env override is enough) would make
/// the same Ethereum block read DAI on one path and USDS on the other, and
/// `token_src_address` would be silently wrong over the gap between them.
///
/// Checking each window's `started_at_block` against `legacy_ethereum_asset`
/// catches both directions — a v10 window starting before the USDS epoch and
/// a v9 window starting after it — without adding a third constant to keep in
/// sync. Home windows carry no `source_asset` and are not affected.
///
/// Both tables are now per deployment, so `chain_id` — always the Foreign
/// chain id here, since only Foreign windows carry a `source_asset` — selects
/// the deployment on both sides of the comparison.
fn assert_epoch_boundaries_agree(
    chain_id: i64,
    started_at_block: u64,
    grammar: &XDaiGrammar,
) -> Result<()> {
    let Some(grammar_asset) = grammar.source_asset else {
        return Ok(());
    };
    let legacy_asset = legacy_ethereum_asset(chain_id, Direction::EthToGno, started_at_block)
        .with_context(|| {
            format!(
                "xDai chain {chain_id} version {:?} window starting at block {started_at_block} \
                 has no legacy Ethereum asset",
                grammar.version
            )
        })?;

    ensure!(
        grammar_asset == legacy_asset,
        "xDai chain {chain_id} Foreign version {:?} starts at block {started_at_block}, where the \
         version grammar table declares source_asset {grammar_asset} but the legacy \
         reconstruction path resolves {legacy_asset}: the same Ethereum block would be labelled \
         with two different assets depending on which path indexed it. Either move this \
         contract's started_at_block in bridges.json back to the side of this deployment's asset \
         boundary that matches its grammar -- on Ethereum that boundary is \
         USDS_EPOCH_START_BLOCK ({USDS_EPOCH_START_BLOCK}) -- or, if the epoch itself moved, \
         update that deployment's constants in indexer/xdai/version.rs together with the grammar \
         table",
        grammar.version
    );

    Ok(())
}

/// Asserts that every subscribed event's `topic0`, as computed from the
/// *configured* ABI, equals the canonical xDai signature's hash for this
/// `(side, version)`. The event-name partition is identical to AMB's, so
/// name-set inference alone cannot separate the two protocols; this is the
/// check that actually does, since an AMB ABI's real selectors differ.
fn assert_canonical_topics(
    chain_id: i64,
    address: Address,
    abi_value: Option<&Value>,
    grammar: &XDaiGrammar,
) -> Result<()> {
    let abi_value = abi_value.with_context(|| {
        format!("missing ABI for xDai contract row chain_id={chain_id} address={address}")
    })?;
    let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
        format!("invalid ABI for xDai contract row chain_id={chain_id} address={address}")
    })?;

    for (event_name, canonical_signature) in grammar.canonical_topics {
        let event = abi
            .events
            .get(*event_name)
            .and_then(|events| events.first())
            .with_context(|| {
                format!("ABI for chain_id={chain_id} address={address} missing event {event_name}")
            })?;
        let expected = keccak256(canonical_signature.as_bytes());
        let found = event.selector();
        ensure!(
            found == expected,
            "xDai ABI for chain_id={chain_id} address={address} event {event_name} has topic0 \
             {found} but the canonical xDai signature `{canonical_signature}` hashes to \
             {expected} -- this looks like an AMB ABI configured under an xDai bridge (or an \
             xDai ABI configured under an AMB bridge)",
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::providers::{Provider, ProviderBuilder};

    use super::*;
    use crate::indexer::xdai::{
        indexer::{XDaiChainConfig, XDaiContractConfig},
        version::{
            CHIADO_EPOCH_FLOOR_BLOCK, ETHEREUM_EPOCH_FLOOR_BLOCK, GNOSIS_EPOCH_FLOOR_BLOCK,
            SEPOLIA_EPOCH_FLOOR_BLOCK, SEPOLIA_MOCK_DAI,
        },
    };

    fn dummy_provider() -> alloy::providers::DynProvider<alloy::network::Ethereum> {
        ProviderBuilder::new()
            .connect_http("http://127.0.0.1:1".parse().unwrap())
            .erased()
    }

    fn foreign_event_abi() -> Value {
        serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}
        ])
    }

    fn home_v7_event_abi() -> Value {
        serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"},{"indexed":false,"name":"token","type":"address"}],"name":"UserRequestForSignature","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}
        ])
    }

    fn home_v6_event_abi() -> Value {
        serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForSignature","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"AffirmationCompleted","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"SignedForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"name":"SignedForUserRequest","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"name":"CollectedSignatures","type":"event"}
        ])
    }

    fn topic_of(abi: &Value, name: &str) -> B256 {
        let parsed: JsonAbi = serde_json::from_value(abi.clone()).expect("valid ABI");
        parsed
            .events
            .get(name)
            .and_then(|events| events.first())
            .expect("event present")
            .selector()
    }

    /// AMB's own AMB proxy ABI: same event *names* on the Foreign side
    /// (`UserRequestForAffirmation`, `RelayedMessage`), different real
    /// signatures/selectors.
    fn amb_foreign_event_abi() -> Value {
        serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"name":"UserRequestForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"name":"RelayedMessage","type":"event"}
        ])
    }

    fn chain_config(chain_id: i64, contracts: Vec<XDaiContractConfig>) -> XDaiChainConfig {
        XDaiChainConfig {
            chain_id,
            provider: dummy_provider(),
            start_block: contracts
                .iter()
                .map(|c| c.started_at_block)
                .min()
                .unwrap_or(0),
            contracts,
        }
    }

    #[test]
    fn side_for_abi_infers_foreign_and_home_from_the_event_set() {
        assert_eq!(
            side_for_abi(1, Address::ZERO, Some(&foreign_event_abi())).unwrap(),
            XDaiSide::Foreign
        );
        assert_eq!(
            side_for_abi(100, Address::ZERO, Some(&home_v7_event_abi())).unwrap(),
            XDaiSide::Home
        );
    }

    #[test]
    fn from_chains_rejects_an_amb_abi_offered_as_xdai() {
        let chains = vec![
            chain_config(
                1,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xAA),
                    version: 9,
                    started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
                    abi: Some(amb_foreign_event_abi()),
                }],
            ),
            chain_config(
                100,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xBB),
                    version: 7,
                    started_at_block: GNOSIS_EPOCH_FLOOR_BLOCK,
                    abi: Some(home_v7_event_abi()),
                }],
            ),
        ];

        let err = AbiRegistry::from_chains(&chains).expect_err("AMB ABI must be rejected as xDai");
        assert!(
            err.to_string().contains("topic0"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn from_chains_accepts_the_real_xdai_event_set() {
        let chains = vec![
            chain_config(
                1,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xAA),
                    version: 9,
                    started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
                    abi: Some(foreign_event_abi()),
                }],
            ),
            chain_config(
                100,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xBB),
                    version: 7,
                    started_at_block: GNOSIS_EPOCH_FLOOR_BLOCK,
                    abi: Some(home_v7_event_abi()),
                }],
            ),
        ];

        let registry = AbiRegistry::from_chains(&chains).expect("real xDai ABI must be accepted");
        assert_eq!(registry.chain_id_for_side(XDaiSide::Foreign).unwrap(), 1);
        assert_eq!(registry.chain_id_for_side(XDaiSide::Home).unwrap(), 100);
    }

    #[test]
    fn from_chains_rejects_a_started_at_block_below_the_epoch_floor() {
        let chains = vec![chain_config(
            1,
            vec![XDaiContractConfig {
                address: Address::repeat_byte(0xAA),
                version: 9,
                started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK - 1,
                abi: Some(foreign_event_abi()),
            }],
        )];

        let err = AbiRegistry::from_chains(&chains).expect_err("below-floor start must fail");
        assert!(
            err.to_string().contains("epoch floor"),
            "unexpected error: {err}"
        );
    }

    /// The version boundary that hides R4 (the DAI→USDS asset flip):
    /// `resolve_log` selects Foreign v9 one block before the upgrade and v10
    /// at the upgrade block, and each version's grammar carries the asset
    /// that was actually held during that window.
    #[test]
    fn foreign_version_boundary_selects_dai_before_usds_from_23748179() {
        let address = Address::repeat_byte(0xCC);
        let chains = vec![chain_config(
            1,
            vec![
                XDaiContractConfig {
                    address,
                    version: 9,
                    started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
                    abi: Some(foreign_event_abi()),
                },
                XDaiContractConfig {
                    address,
                    version: 10,
                    started_at_block: 23_748_179,
                    abi: Some(foreign_event_abi()),
                },
            ],
        )];
        let registry = AbiRegistry::from_chains(&chains).expect("registry builds");
        let topic = topic_of(&foreign_event_abi(), "UserRequestForAffirmation");

        let dai = alloy::primitives::address!("6B175474E89094C44Da98b954EedeAC495271d0F");
        let usds = alloy::primitives::address!("dC035D45d973E3EC169d2276DDab16f1e407384F");

        let before = match registry.resolve_log(1, address, &topic, 23_748_178) {
            LogResolution::Matched(_, kind) => kind,
            other => panic!("expected a match at 23748178, got {other:?}"),
        };
        let after = match registry.resolve_log(1, address, &topic, 23_748_179) {
            LogResolution::Matched(_, kind) => kind,
            other => panic!("expected a match at 23748179, got {other:?}"),
        };

        assert_eq!(before.version, 9);
        assert_eq!(after.version, 10);
        assert_eq!(
            grammar_for(1, XDaiSide::Foreign, before.version)
                .unwrap()
                .source_asset,
            Some(dai)
        );
        assert_eq!(
            grammar_for(1, XDaiSide::Foreign, after.version)
                .unwrap()
                .source_asset,
            Some(usds)
        );
    }

    /// The Home-side counterpart: `UserRequestForSignature` changes topic0 at
    /// the v7 upgrade (it gains `token`), so the version -- and therefore the
    /// grammar's `blob_layout` -- must flip exactly at block 43027713.
    #[test]
    fn home_version_boundary_selects_v6_before_v7_from_43027713() {
        let address = Address::repeat_byte(0xDD);
        let chains = vec![chain_config(
            100,
            vec![
                XDaiContractConfig {
                    address,
                    version: 6,
                    started_at_block: GNOSIS_EPOCH_FLOOR_BLOCK,
                    abi: Some(home_v6_event_abi()),
                },
                XDaiContractConfig {
                    address,
                    version: 7,
                    started_at_block: 43_027_713,
                    abi: Some(home_v7_event_abi()),
                },
            ],
        )];
        let registry = AbiRegistry::from_chains(&chains).expect("registry builds");
        let v6_topic = topic_of(&home_v6_event_abi(), "UserRequestForSignature");
        let v7_topic = topic_of(&home_v7_event_abi(), "UserRequestForSignature");

        assert!(matches!(
            registry.resolve_log(100, address, &v6_topic, 43_027_712),
            LogResolution::Matched(_, ContractKind { version: 6, .. })
        ));
        assert!(matches!(
            registry.resolve_log(100, address, &v7_topic, 43_027_713),
            LogResolution::Matched(_, ContractKind { version: 7, .. })
        ));
    }

    const SEPOLIA: i64 = 11_155_111;
    const CHIADO: i64 = 10_200;

    /// The Sepolia/Chiado deployment exactly as `config/full-testnet` declares
    /// it: the proxies' own `version()` counters (Foreign 2, Home 3), each
    /// window starting at its deployment's epoch floor.
    fn testnet_chains() -> Vec<XDaiChainConfig> {
        vec![
            chain_config(
                SEPOLIA,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xAA),
                    version: 2,
                    started_at_block: SEPOLIA_EPOCH_FLOOR_BLOCK,
                    abi: Some(foreign_event_abi()),
                }],
            ),
            chain_config(
                CHIADO,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xBB),
                    version: 3,
                    started_at_block: CHIADO_EPOCH_FLOOR_BLOCK,
                    abi: Some(home_v7_event_abi()),
                }],
            ),
        ]
    }

    /// B1: the registry's side map is the only source of xDai chain ids, so a
    /// non-mainnet Foreign/Home pair resolves to itself rather than to 1/100.
    #[test]
    fn chain_ids_resolve_a_non_mainnet_foreign_home_pair() {
        let registry = AbiRegistry::from_chains(&testnet_chains()).expect("registry builds");

        assert_eq!(
            registry.chain_id_for_side(XDaiSide::Foreign).unwrap(),
            SEPOLIA
        );
        assert_eq!(
            registry.counterpart_chain_id(XDaiSide::Foreign).unwrap(),
            CHIADO
        );
        assert_eq!(
            registry.counterpart_chain_id(XDaiSide::Home).unwrap(),
            SEPOLIA
        );
        assert_eq!(
            registry.chain_ids().unwrap(),
            ChainIds {
                foreign: SEPOLIA,
                home: CHIADO
            }
        );
    }

    fn foreign_chains_with_window(version: i16, started_at_block: u64) -> Vec<XDaiChainConfig> {
        vec![chain_config(
            1,
            vec![XDaiContractConfig {
                address: Address::repeat_byte(0xAA),
                version,
                started_at_block,
                abi: Some(foreign_event_abi()),
            }],
        )]
    }

    /// B2: the config's Foreign v10 boundary and `USDS_EPOCH_START_BLOCK` are
    /// two independent tables that must name the same block. This is the
    /// passing case — the boundary the shipped `bridges.json` actually uses.
    #[test]
    fn from_chains_accepts_foreign_windows_that_agree_with_the_usds_epoch() {
        AbiRegistry::from_chains(&foreign_chains_with_window(9, ETHEREUM_EPOCH_FLOOR_BLOCK))
            .expect("a v9 window below the USDS epoch is consistent");
        AbiRegistry::from_chains(&foreign_chains_with_window(10, USDS_EPOCH_START_BLOCK))
            .expect("a v10 window at the USDS epoch is consistent");
    }

    /// A v10 (USDS) window starting before the USDS epoch: the grammar would
    /// label the gap USDS while legacy reconstruction labels it DAI.
    #[test]
    fn from_chains_rejects_a_usds_window_that_starts_before_the_usds_epoch() {
        let err =
            AbiRegistry::from_chains(&foreign_chains_with_window(10, USDS_EPOCH_START_BLOCK - 1))
                .expect_err("a v10 window below the USDS epoch must fail");
        let message = err.to_string();
        assert!(
            message.contains("USDS_EPOCH_START_BLOCK"),
            "the error must name the constant to change: {message}"
        );
        assert!(
            message.contains("started_at_block"),
            "the error must name the config field to change: {message}"
        );
    }

    /// The opposite direction: a v9 (DAI) window starting at or after the USDS
    /// epoch, which legacy reconstruction would label USDS.
    #[test]
    fn from_chains_rejects_a_dai_window_that_starts_at_or_after_the_usds_epoch() {
        let err = AbiRegistry::from_chains(&foreign_chains_with_window(9, USDS_EPOCH_START_BLOCK))
            .expect_err("a v9 window at the USDS epoch must fail");
        assert!(
            err.to_string().contains("USDS_EPOCH_START_BLOCK"),
            "unexpected error: {err}"
        );
    }

    /// The whole testnet deployment builds: both sides' ABIs are the mainnet
    /// ones verbatim (identical `topic0`s), the proxy version counters 2/3
    /// select the Sepolia/Chiado grammars, and both windows sit exactly on
    /// their epoch floors.
    #[test]
    fn from_chains_accepts_the_sepolia_chiado_deployment() {
        let registry = AbiRegistry::from_chains(&testnet_chains()).expect("testnet config builds");

        assert_eq!(
            registry.chain_id_for_side(XDaiSide::Foreign).unwrap(),
            SEPOLIA
        );
        assert_eq!(registry.chain_id_for_side(XDaiSide::Home).unwrap(), CHIADO);

        let topic = topic_of(&foreign_event_abi(), "UserRequestForAffirmation");
        let kind =
            match registry.resolve_log(SEPOLIA, Address::repeat_byte(0xAA), &topic, 8_261_069) {
                LogResolution::Matched(_, kind) => kind,
                other => panic!("expected a match at the first nonce-era deposit, got {other:?}"),
            };
        assert_eq!(kind.version, 2);
        assert_eq!(
            grammar_for(SEPOLIA, XDaiSide::Foreign, kind.version)
                .unwrap()
                .source_asset,
            Some(SEPOLIA_MOCK_DAI)
        );
    }

    /// The floor no longer works by excluding hash-keyed completions from the
    /// scan window: a hash-keyed `AffirmationCompleted`/`RelayedMessage`
    /// inside the window now resolves to its canonical identity from receipt
    /// evidence (`events.rs::decode_source_evidence`), and multiple
    /// destination executions under one canonical identity are handled as
    /// multiple-execution anomalies. The floor's only remaining job is to
    /// mark where this deployment's nonce identity epoch begins, so it must
    /// sit at or below every known nonce-keyed completion in the documented
    /// window and is shared by both registered Chiado Home windows.
    #[test]
    fn the_chiado_floor_admits_every_known_window_completion() {
        let floor_v2 = grammar_for(CHIADO, XDaiSide::Home, 2)
            .expect("the Chiado v2 grammar is registered")
            .epoch_floor_block;
        let floor_v3 = grammar_for(CHIADO, XDaiSide::Home, 3)
            .expect("the Chiado v3 grammar is registered")
            .epoch_floor_block;
        assert_eq!(floor_v2, floor_v3, "both Chiado windows share one floor");

        // Every known `AffirmationCompleted` in the open Chiado window
        // `[15562365, 20800000]` on proxy `0xccA0Dc2A058884e62082312F09541cC7566406f0`
        // (`.memory-bank/research/xdai-bridge-sepolia-chiado-upgrade-history.md`):
        // nonce 0, nonce 1, four completions sharing block 16803580 (two
        // raw-hash, one nonce-0 alias, one nonce-1 alias), the nonce-2
        // negative control, and both nonce-3 completions. The literal floor
        // value itself is pinned elsewhere
        // (`chiado_epoch_floor_is_shared_by_both_home_windows`,
        // `testnet_grammar_windows_resolve_sepolia_and_chiado_values`); this
        // test's job is only to prove the floor does not exclude any of them.
        const KNOWN_WINDOW_COMPLETION_BLOCKS: [u64; 6] = [
            15_612_527, // nonce 0
            15_612_593, // nonce 1
            16_803_580, // four completions, one block
            18_042_501, // nonce 2 (single completion, negative control)
            20_553_477, // nonce 3 (alias)
            20_706_963, // nonce 3
        ];
        for block in KNOWN_WINDOW_COMPLETION_BLOCKS {
            assert!(
                floor_v2 <= block,
                "the floor must admit known window completion at block {block}"
            );
        }
    }

    /// Both Chiado Home windows share one proxy address; `resolve_log` must
    /// still select the version whose grammar matches the source event's
    /// actual argument count at that block -- the three-argument
    /// `UserRequestForSignature` below the v3 boundary, the four-argument one
    /// at and above it.
    #[test]
    fn both_chiado_windows_on_one_address_select_their_own_source_topic_by_block() {
        const V3_STARTED_AT_BLOCK: u64 = 20_553_827;
        let address = Address::repeat_byte(0xBB);
        let chains = vec![chain_config(
            CHIADO,
            vec![
                XDaiContractConfig {
                    address,
                    version: 2,
                    started_at_block: CHIADO_EPOCH_FLOOR_BLOCK,
                    abi: Some(home_v6_event_abi()),
                },
                XDaiContractConfig {
                    address,
                    version: 3,
                    started_at_block: V3_STARTED_AT_BLOCK,
                    abi: Some(home_v7_event_abi()),
                },
            ],
        )];
        let registry = AbiRegistry::from_chains(&chains).expect("registry builds");
        let v6_topic = topic_of(&home_v6_event_abi(), "UserRequestForSignature");
        let v7_topic = topic_of(&home_v7_event_abi(), "UserRequestForSignature");

        assert!(matches!(
            registry.resolve_log(CHIADO, address, &v6_topic, V3_STARTED_AT_BLOCK - 1),
            LogResolution::Matched(_, ContractKind { version: 2, .. })
        ));
        assert!(matches!(
            registry.resolve_log(CHIADO, address, &v7_topic, V3_STARTED_AT_BLOCK),
            LogResolution::Matched(_, ContractKind { version: 3, .. })
        ));
    }

    /// A testnet `started_at_block` below its own deployment's floor is
    /// rejected, and the error names the testnet floor rather than mainnet's.
    #[test]
    fn from_chains_rejects_a_testnet_start_below_the_testnet_floor() {
        let chains = vec![chain_config(
            SEPOLIA,
            vec![XDaiContractConfig {
                address: Address::repeat_byte(0xAA),
                version: 2,
                started_at_block: SEPOLIA_EPOCH_FLOOR_BLOCK - 1,
                abi: Some(foreign_event_abi()),
            }],
        )];

        let err = AbiRegistry::from_chains(&chains).expect_err("below-floor start must fail");
        let message = err.to_string();
        assert!(
            message.contains("epoch floor"),
            "unexpected error: {message}"
        );
        assert!(
            message.contains(&SEPOLIA_EPOCH_FLOOR_BLOCK.to_string()),
            "the error must name this deployment's own floor: {message}"
        );
    }

    /// The deployment-isolation guarantee, at the level an operator would hit
    /// it: a mainnet chain id with a testnet proxy version is a hard startup
    /// error, not a silent selection of the testnet floor and mock-DAI asset.
    #[test]
    fn from_chains_rejects_a_testnet_version_under_a_mainnet_chain_id() {
        let chains = vec![chain_config(
            1,
            vec![XDaiContractConfig {
                address: Address::repeat_byte(0xAA),
                version: 2,
                started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
                abi: Some(foreign_event_abi()),
            }],
        )];

        let err = AbiRegistry::from_chains(&chains)
            .expect_err("a testnet version on Ethereum must be rejected");
        assert!(
            err.to_string().contains("no xDai grammar registered"),
            "unexpected error: {err}"
        );
    }

    // --- the shipped config files, built exactly as the server builds them ---

    /// Reads the one `type == "xdai"` bridge out of a real `bridges.json` and
    /// turns it into the `XDaiChainConfig`s `build_xdai_chain_configs` would
    /// produce. Deliberately reads the file rather than a fixture: the point
    /// is to prove the *shipped* config resolves the values it is supposed to.
    fn chains_from_shipped_config(relative_path: &str) -> Vec<XDaiChainConfig> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("manifest dir has a parent")
            .join(relative_path);
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
        let bridges: Vec<Value> = serde_json::from_str(&raw).expect("bridges.json parses");
        let bridge = bridges
            .iter()
            .find(|bridge| bridge["type"] == "xdai")
            .unwrap_or_else(|| panic!("{relative_path} has no xdai bridge"));

        let mut by_chain: std::collections::BTreeMap<i64, Vec<XDaiContractConfig>> =
            Default::default();
        for contract in bridge["contracts"].as_array().expect("contracts array") {
            let chain_id = contract["chain_id"].as_i64().expect("chain_id");
            let abi: Value = serde_json::from_str(contract["abi"].as_str().expect("abi string"))
                .expect("abi parses");
            by_chain
                .entry(chain_id)
                .or_default()
                .push(XDaiContractConfig {
                    address: contract["address"]
                        .as_str()
                        .expect("address")
                        .parse()
                        .expect("address parses"),
                    version: contract["version"].as_i64().expect("version") as i16,
                    started_at_block: contract["started_at_block"].as_u64().expect("start"),
                    abi: Some(abi),
                });
        }

        by_chain
            .into_iter()
            .map(|(chain_id, contracts)| chain_config(chain_id, contracts))
            .collect()
    }

    fn source_asset_at(registry: &AbiRegistry, chain_id: i64, block: u64) -> Option<Address> {
        let topic = topic_of(&foreign_event_abi(), "UserRequestForAffirmation");
        let address = registry.foreign_proxy_address().expect("foreign address");
        match registry.resolve_log(chain_id, address, &topic, block) {
            LogResolution::Matched(_, kind) => {
                grammar_for(chain_id, XDaiSide::Foreign, kind.version)
                    .expect("grammar for a registered window")
                    .source_asset
            }
            other => panic!("expected a match at {chain_id}/{block}, got {other:?}"),
        }
    }

    /// The regression guard for the whole change: the shipped mainnet config
    /// must still resolve the exact floors, assets and epoch boundary it did
    /// before the grammar table became multi-deployment.
    #[rstest::rstest]
    #[case("config/xdai/bridges.json")]
    #[case("config/full-mainnet/bridges.json")]
    fn the_shipped_mainnet_config_resolves_unchanged_floors_and_assets(#[case] path: &str) {
        let chains = chains_from_shipped_config(path);
        let registry = AbiRegistry::from_chains(&chains).expect("the shipped config must build");

        assert_eq!(registry.chain_id_for_side(XDaiSide::Foreign).unwrap(), 1);
        assert_eq!(registry.chain_id_for_side(XDaiSide::Home).unwrap(), 100);

        // Floors, as the `ensure!` in `from_chains` sees them.
        for contract in &chains[0].contracts {
            assert_eq!(
                grammar_for(1, XDaiSide::Foreign, contract.version)
                    .unwrap()
                    .epoch_floor_block,
                22_273_407
            );
        }
        for contract in &chains[1].contracts {
            assert_eq!(
                grammar_for(100, XDaiSide::Home, contract.version)
                    .unwrap()
                    .epoch_floor_block,
                39_569_937
            );
        }
        assert_eq!(
            chains[0].contracts.iter().map(|c| c.started_at_block).min(),
            Some(22_273_407)
        );
        assert_eq!(
            chains[1].contracts.iter().map(|c| c.started_at_block).min(),
            Some(39_569_937)
        );

        // The DAI -> USDS boundary, from both independent paths.
        let dai = alloy::primitives::address!("6B175474E89094C44Da98b954EedeAC495271d0F");
        let usds = alloy::primitives::address!("dC035D45d973E3EC169d2276DDab16f1e407384F");
        assert_eq!(source_asset_at(&registry, 1, 23_748_178), Some(dai));
        assert_eq!(source_asset_at(&registry, 1, 23_748_179), Some(usds));
        assert_eq!(
            legacy_ethereum_asset(1, Direction::EthToGno, 23_748_178).unwrap(),
            dai
        );
        assert_eq!(
            legacy_ethereum_asset(1, Direction::EthToGno, 23_748_179).unwrap(),
            usds
        );
        assert!(
            legacy_ethereum_asset(1, Direction::EthToGno, 9_161_002).is_err(),
            "the pre-DAI-epoch bail must survive"
        );
    }

    /// The counterpart for the testnet sets: the shipped config builds and
    /// resolves testnet values, not mainnet ones. Both files must stay in
    /// step -- `config/xdai/bridges-testnet.json` is the xDai-only subset of
    /// `config/full-testnet/bridges.json`, exactly as `config/omnibridge`'s
    /// testnet pair is of the AMB one.
    #[rstest::rstest]
    #[case("config/full-testnet/bridges.json")]
    #[case("config/xdai/bridges-testnet.json")]
    fn the_shipped_testnet_config_resolves_sepolia_and_chiado_values(#[case] path: &str) {
        let chains = chains_from_shipped_config(path);
        let registry =
            AbiRegistry::from_chains(&chains).expect("the shipped testnet config must build");

        assert_eq!(
            registry.chain_ids().unwrap(),
            ChainIds {
                foreign: SEPOLIA,
                home: CHIADO
            }
        );

        let foreign = chains
            .iter()
            .find(|chain| chain.chain_id == SEPOLIA)
            .expect("Sepolia chain");
        assert_eq!(foreign.contracts.len(), 1);
        assert_eq!(foreign.contracts[0].version, 2);
        assert_eq!(foreign.contracts[0].started_at_block, 8_239_484);

        let home = chains
            .iter()
            .find(|chain| chain.chain_id == CHIADO)
            .expect("Chiado chain");
        assert_eq!(
            home.contracts.len(),
            2,
            "Chiado now has two Home windows on one proxy address: v2 (grammar boundary) and \
             v3 (token argument added)"
        );
        let home_v2 = home
            .contracts
            .iter()
            .find(|c| c.version == 2)
            .expect("Chiado v2 contract");
        assert_eq!(home_v2.started_at_block, 15_562_365);
        let home_v3 = home
            .contracts
            .iter()
            .find(|c| c.version == 3)
            .expect("Chiado v3 contract");
        assert_eq!(home_v3.started_at_block, 20_553_827);

        // The first real deposit resolves to the mock DAI, not to mainnet DAI.
        assert_eq!(
            source_asset_at(&registry, SEPOLIA, 8_261_069),
            Some(SEPOLIA_MOCK_DAI)
        );
        assert_eq!(
            legacy_ethereum_asset(SEPOLIA, Direction::EthToGno, 23_748_179).unwrap(),
            SEPOLIA_MOCK_DAI
        );
    }

    #[test]
    fn foreign_proxy_address_returns_the_configured_address() {
        let address = Address::repeat_byte(0xEE);
        let chains = vec![chain_config(
            1,
            vec![
                XDaiContractConfig {
                    address,
                    version: 9,
                    started_at_block: ETHEREUM_EPOCH_FLOOR_BLOCK,
                    abi: Some(foreign_event_abi()),
                },
                XDaiContractConfig {
                    address,
                    version: 10,
                    started_at_block: 23_748_179,
                    abi: Some(foreign_event_abi()),
                },
            ],
        )];
        let registry = AbiRegistry::from_chains(&chains).expect("registry builds");

        assert_eq!(registry.foreign_proxy_address().unwrap(), address);
    }
}
