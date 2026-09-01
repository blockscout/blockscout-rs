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
    version::{
        FOREIGN_EPOCH_FLOOR_BLOCK, FOREIGN_EVENTS, HOME_EPOCH_FLOOR_BLOCK, HOME_EVENTS,
        XDaiGrammar, XDaiSide, grammar_for,
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

                let grammar = grammar_for(side, contract.version)?;
                debug_assert_eq!(
                    grammar.side, side,
                    "grammar_for returned a grammar registered under the wrong side"
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
                // no on-chain signal. See ADR-006 / the protocol primer.
                let floor = match side {
                    XDaiSide::Foreign => FOREIGN_EPOCH_FLOOR_BLOCK,
                    XDaiSide::Home => HOME_EPOCH_FLOOR_BLOCK,
                };
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

    // `counterpart_chain_id` / `side_for_chain` / `event_for_log` are not
    // needed until phase B2 (the Gno→Eth direction correlates
    // `SignedForUserRequest`/`CollectedSignatures` by the emitting chain's
    // side, and scans receipt logs for a matching event the way AMB's
    // `find_tokens_bridged` does). Add them there rather than now: xDai's
    // "direction is derived, never looked up" design means B1's Eth→Gno
    // handlers need neither.

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
    use crate::indexer::xdai::indexer::{XDaiChainConfig, XDaiContractConfig};

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
                    started_at_block: FOREIGN_EPOCH_FLOOR_BLOCK,
                    abi: Some(amb_foreign_event_abi()),
                }],
            ),
            chain_config(
                100,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xBB),
                    version: 7,
                    started_at_block: HOME_EPOCH_FLOOR_BLOCK,
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
                    started_at_block: FOREIGN_EPOCH_FLOOR_BLOCK,
                    abi: Some(foreign_event_abi()),
                }],
            ),
            chain_config(
                100,
                vec![XDaiContractConfig {
                    address: Address::repeat_byte(0xBB),
                    version: 7,
                    started_at_block: HOME_EPOCH_FLOOR_BLOCK,
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
                started_at_block: FOREIGN_EPOCH_FLOOR_BLOCK - 1,
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
                    started_at_block: FOREIGN_EPOCH_FLOOR_BLOCK,
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
            grammar_for(XDaiSide::Foreign, before.version)
                .unwrap()
                .source_asset,
            Some(dai)
        );
        assert_eq!(
            grammar_for(XDaiSide::Foreign, after.version)
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
                    started_at_block: HOME_EPOCH_FLOOR_BLOCK,
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
}
