use std::collections::HashMap;

use alloy::{
    json_abi::{Event, JsonAbi},
    primitives::{Address, B256, keccak256},
    rpc::types::Filter,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

use crate::indexer::evm::abi_registry;

use super::{
    indexer::AmbChainConfig,
    version::{AmbGrammar, AmbSide, HeaderLayout, amb_grammar_for, mediator_grammar_for},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContractKind {
    AmbProxy {
        side: AmbSide,
        header_layout: HeaderLayout,
    },
    OmnibridgeMediator,
}

// Test-only: production code never names these directly (`from_chains`
// builds the registry through the generic `insert_contract`), but AMB's own
// event/indexer tests construct fixture registries with them. Gated so a
// non-test build does not see them as dead code.
#[cfg(test)]
pub(crate) type ContractVersion = abi_registry::ContractVersion<ContractKind>;
#[cfg(test)]
pub(crate) type ContractAbi = abi_registry::ContractAbi<ContractKind>;

/// What a fetched log resolved to; see
/// [`abi_registry::LogResolution`] for the full contract.
pub(crate) type LogResolution<'a> = abi_registry::LogResolution<'a, ContractKind>;

/// Thin AMB-specific wrapper around the protocol-agnostic
/// [`abi_registry::AbiRegistry`]: the version-window resolution (ADR-006) is
/// shared with xDai, while the Home/Foreign side map below is AMB-only — xDai
/// has a fixed two-chain topology and needs no equivalent.
#[derive(Clone, Debug, Default)]
pub(crate) struct AbiRegistry {
    inner: abi_registry::AbiRegistry<ContractKind>,
    chain_by_side: HashMap<AmbSide, i64>,
}

impl AbiRegistry {
    #[cfg(test)]
    pub(crate) fn from_contracts_for_test(contracts: Vec<ContractAbi>) -> Self {
        Self {
            inner: abi_registry::AbiRegistry::from_contracts_for_test(contracts),
            chain_by_side: HashMap::new(),
        }
    }

    pub(crate) fn from_chains(chains: &[AmbChainConfig]) -> Result<Self> {
        let mut registry = Self::default();

        for chain in chains {
            ensure!(
                !chain.amb_proxies.is_empty(),
                "AMB chain {} has no amb_proxy contract",
                chain.chain_id
            );

            // The side is a property of the chain, not of a version: an
            // upgrade cannot turn a Home proxy into a Foreign one. Every
            // configured version must agree, or the config describes two
            // different bridges under one chain id.
            let mut chain_side: Option<AmbSide> = None;

            for proxy in &chain.amb_proxies {
                let grammar = amb_grammar_for(proxy.version)?;
                let side =
                    amb_side_for_abi(chain.chain_id, proxy.address, proxy.abi.as_ref(), grammar)?;

                // xDai's proxy events share the exact same name partition, so
                // the name-set inference above cannot tell the two protocols
                // apart on its own — only the canonical topic0 can. This is
                // what stops an xDai ABI from being accepted as AMB (or vice
                // versa) under a plausible-looking config.
                let canonical_topics = match side {
                    AmbSide::Foreign => grammar.foreign_canonical_topics,
                    AmbSide::Home => grammar.home_canonical_topics,
                };
                assert_canonical_topics(
                    chain.chain_id,
                    proxy.address,
                    proxy.abi.as_ref(),
                    canonical_topics,
                )?;

                match chain_side {
                    None => chain_side = Some(side),
                    Some(existing) => ensure!(
                        existing == side,
                        "AMB chain {} has amb_proxy versions on different sides ({existing:?} and {side:?})",
                        chain.chain_id
                    ),
                }

                let events = match side {
                    AmbSide::Foreign => grammar.foreign_events,
                    AmbSide::Home => grammar.home_events,
                };
                registry.inner.insert_contract(
                    chain.chain_id,
                    proxy.address,
                    proxy.started_at_block,
                    ContractKind::AmbProxy {
                        side,
                        header_layout: grammar.header_layout,
                    },
                    proxy.abi.as_ref(),
                    events,
                )?;

                // The registered window is what every later decode depends on,
                // and it is derived from config rather than observed, so it is
                // worth being able to read back from the log.
                tracing::debug!(
                    chain_id = chain.chain_id,
                    address = %proxy.address,
                    grammar_version = ?grammar.version,
                    started_at_block = proxy.started_at_block,
                    side = ?side,
                    "registered AMB proxy version"
                );
            }

            let side = chain_side.expect("non-empty amb_proxies yields a side");
            ensure!(
                registry
                    .chain_by_side
                    .insert(side, chain.chain_id)
                    .is_none(),
                "AMB bridge config has multiple {side:?} chains"
            );

            // Mediators are optional: a bridge configured with AMB contracts
            // alone indexes messages without token transfers.
            for mediator in &chain.mediators {
                let grammar = mediator_grammar_for(mediator.version)?;
                registry.inner.insert_contract(
                    chain.chain_id,
                    mediator.address,
                    mediator.started_at_block,
                    ContractKind::OmnibridgeMediator,
                    mediator.abi.as_ref(),
                    grammar.events,
                )?;

                tracing::debug!(
                    chain_id = chain.chain_id,
                    address = %mediator.address,
                    grammar_version = ?grammar.version,
                    started_at_block = mediator.started_at_block,
                    "registered Omnibridge mediator version"
                );
            }
        }

        Ok(registry)
    }

    pub(crate) fn chain_id_for_side(&self, side: AmbSide) -> Result<i64> {
        self.chain_by_side
            .get(&side)
            .copied()
            .with_context(|| format!("AMB bridge config missing {side:?} chain"))
    }

    pub(crate) fn counterpart_chain_id(&self, side: AmbSide) -> Result<i64> {
        let counterpart = match side {
            AmbSide::Foreign => AmbSide::Home,
            AmbSide::Home => AmbSide::Foreign,
        };
        self.chain_id_for_side(counterpart)
    }

    pub(crate) fn side_for_chain(&self, chain_id: i64) -> Result<AmbSide> {
        self.chain_by_side
            .iter()
            .find_map(|(side, configured_chain_id)| {
                (*configured_chain_id == chain_id).then_some(*side)
            })
            .with_context(|| format!("AMB bridge config missing side for chain {chain_id}"))
    }

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

    pub(crate) fn event_for_log(
        &self,
        chain_id: i64,
        address: Address,
        topic: &B256,
        block_number: u64,
    ) -> Option<(&Event, ContractKind)> {
        self.inner
            .event_for_log(chain_id, address, topic, block_number)
    }

    pub(crate) fn filter_for_chain(&self, chain_id: i64) -> Result<Filter> {
        self.inner.filter_for_chain(chain_id)
    }
}

fn amb_side_for_abi(
    chain_id: i64,
    address: Address,
    abi_value: Option<&Value>,
    grammar: &AmbGrammar,
) -> Result<AmbSide> {
    let abi_value = abi_value.with_context(|| {
        format!("missing ABI for AMB contract row chain_id={chain_id} address={address}")
    })?;
    let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
        format!("invalid ABI for AMB contract row chain_id={chain_id} address={address}")
    })?;

    let has_foreign_events = grammar
        .foreign_events
        .iter()
        .all(|event_name| abi.events.contains_key(*event_name));
    let has_home_events = grammar
        .home_events
        .iter()
        .all(|event_name| abi.events.contains_key(*event_name));

    match (has_foreign_events, has_home_events) {
        (true, false) => Ok(AmbSide::Foreign),
        (false, true) => Ok(AmbSide::Home),
        (true, true) => bail!(
            "AMB ABI for chain_id={chain_id} address={address} contains both Home and Foreign event sets"
        ),
        (false, false) => bail!(
            "AMB ABI for chain_id={chain_id} address={address} does not match a Home or Foreign event set"
        ),
    }
}

/// Asserts that every subscribed event's `topic0`, as computed from the
/// *configured* ABI, equals the canonical AMB signature's hash. Mirrors
/// `xdai::abi::assert_canonical_topics`: the event-name partition is
/// identical between the two protocols, so name-set inference alone cannot
/// separate them — only the canonical topic0 can.
fn assert_canonical_topics(
    chain_id: i64,
    address: Address,
    abi_value: Option<&Value>,
    canonical_topics: &[(&str, &str)],
) -> Result<()> {
    let abi_value = abi_value.with_context(|| {
        format!("missing ABI for AMB contract row chain_id={chain_id} address={address}")
    })?;
    let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
        format!("invalid ABI for AMB contract row chain_id={chain_id} address={address}")
    })?;

    for (event_name, canonical_signature) in canonical_topics {
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
            "AMB ABI for chain_id={chain_id} address={address} event {event_name} has topic0 \
             {found} but the canonical AMB signature `{canonical_signature}` hashes to \
             {expected} -- this looks like an xDai ABI configured under an AMB bridge (or an \
             AMB ABI configured under an xDai bridge)",
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloy::primitives::Address;

    use crate::indexer::amb::version::{AmbSide, amb_grammar_for};

    use super::amb_side_for_abi;

    #[test]
    fn amb_side_for_abi_infers_side_from_configured_event_set() {
        let address = Address::from([2; 20]);
        let grammar = amb_grammar_for(6).expect("grammar");
        let foreign_abi = serde_json::json!([
            {"type":"event","name":"UserRequestForAffirmation","inputs":[],"anonymous":false},
            {"type":"event","name":"RelayedMessage","inputs":[],"anonymous":false}
        ]);
        let home_abi = serde_json::json!([
            {"type":"event","name":"UserRequestForSignature","inputs":[],"anonymous":false},
            {"type":"event","name":"AffirmationCompleted","inputs":[],"anonymous":false},
            {"type":"event","name":"SignedForAffirmation","inputs":[],"anonymous":false},
            {"type":"event","name":"SignedForUserRequest","inputs":[],"anonymous":false},
            {"type":"event","name":"CollectedSignatures","inputs":[],"anonymous":false}
        ]);

        assert_eq!(
            amb_side_for_abi(11155111, address, Some(&foreign_abi), grammar).expect("foreign side"),
            AmbSide::Foreign
        );
        assert_eq!(
            amb_side_for_abi(10200, address, Some(&home_abi), grammar).expect("home side"),
            AmbSide::Home
        );
    }

    /// End-to-end over the config shape a versioned deployment actually has:
    /// two mediator versions behind one address, and a chain with no mediator
    /// at all. Both used to be impossible — the first because only one entry
    /// per kind reached the registry, the second because a mediator was
    /// mandatory.
    #[test]
    fn from_chains_registers_several_versions_of_one_address_and_allows_no_mediator() {
        use super::AbiRegistry;
        use crate::indexer::amb::indexer::{AmbChainConfig, AmbContractConfig};
        use alloy::providers::{Provider, ProviderBuilder};

        let home_abi = serde_json::json!([
            {"type":"event","name":"UserRequestForSignature","inputs":[{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"encodedData","type":"bytes"}],"anonymous":false},
            {"type":"event","name":"AffirmationCompleted","inputs":[{"indexed":true,"name":"sender","type":"address"},{"indexed":true,"name":"executor","type":"address"},{"indexed":true,"name":"messageId","type":"bytes32"},{"indexed":false,"name":"status","type":"bool"}],"anonymous":false},
            {"type":"event","name":"SignedForAffirmation","inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"anonymous":false},
            {"type":"event","name":"SignedForUserRequest","inputs":[{"indexed":true,"name":"signer","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"}],"anonymous":false},
            {"type":"event","name":"CollectedSignatures","inputs":[{"indexed":false,"name":"authorityResponsibleForRelay","type":"address"},{"indexed":false,"name":"messageHash","type":"bytes32"},{"indexed":false,"name":"NumberOfCollectedSignatures","type":"uint256"}],"anonymous":false}
        ]);
        let mediator_abi = serde_json::json!([
            {"type":"event","name":"TokensBridgingInitiated","inputs":[],"anonymous":false},
            {"type":"event","name":"TokensBridged","inputs":[],"anonymous":false},
            {"type":"event","name":"NewTokenRegistered","inputs":[],"anonymous":false},
            {"type":"event","name":"FailedMessageFixed","inputs":[],"anonymous":false}
        ]);
        let proxy_address = Address::from([1; 20]);
        let mediator_address = Address::from([2; 20]);
        let provider = ProviderBuilder::new()
            .connect_http("http://127.0.0.1:1".parse().unwrap())
            .erased();

        let chains = vec![AmbChainConfig {
            chain_id: 100,
            provider,
            start_block: 100,
            amb_proxies: vec![AmbContractConfig {
                address: proxy_address,
                version: 6,
                started_at_block: 100,
                abi: Some(home_abi),
            }],
            mediators: vec![
                AmbContractConfig {
                    address: mediator_address,
                    version: 6,
                    started_at_block: 100,
                    abi: Some(mediator_abi.clone()),
                },
                AmbContractConfig {
                    address: mediator_address,
                    version: 8,
                    started_at_block: 5_000,
                    abi: Some(mediator_abi),
                },
            ],
        }];

        let registry = AbiRegistry::from_chains(&chains).expect("registry builds");

        let mediator = registry
            .inner
            .contracts
            .get(&(100, mediator_address))
            .expect("mediator registered");
        assert_eq!(
            mediator
                .versions
                .iter()
                .map(|version| version.started_at_block)
                .collect::<Vec<_>>(),
            vec![100, 5_000],
            "both mediator versions must survive; only one used to reach the registry"
        );

        // Same chain, mediator dropped: still a valid registry.
        let mut without_mediator = chains;
        without_mediator[0].mediators.clear();
        let registry = AbiRegistry::from_chains(&without_mediator)
            .expect("a chain with no mediator must still build");
        assert!(registry.inner.contracts.contains_key(&(100, proxy_address)));
        assert!(
            !registry
                .inner
                .contracts
                .contains_key(&(100, mediator_address))
        );
    }

    /// The reverse of `xdai::abi::from_chains_rejects_an_amb_abi_offered_as_xdai`:
    /// xDai's proxy events share the exact same name partition as AMB's, so
    /// this exercises the direction that name-set inference alone cannot
    /// catch — only `assert_canonical_topics` does.
    #[test]
    fn from_chains_rejects_an_xdai_abi_offered_as_amb() {
        use super::AbiRegistry;
        use crate::indexer::amb::indexer::{AmbChainConfig, AmbContractConfig};
        use alloy::providers::{Provider, ProviderBuilder};

        // Real xDai Foreign/Home event ABIs (see xdai::abi's own fixtures):
        // same event names as AMB, different real signatures/selectors.
        let xdai_foreign_abi = serde_json::json!([
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"nonce","type":"bytes32"}],"name":"UserRequestForAffirmation","type":"event"},
            {"anonymous":false,"inputs":[{"indexed":false,"name":"recipient","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":false,"name":"transactionHash","type":"bytes32"}],"name":"RelayedMessage","type":"event"}
        ]);

        let provider = ProviderBuilder::new()
            .connect_http("http://127.0.0.1:1".parse().unwrap())
            .erased();
        let chains = vec![AmbChainConfig {
            chain_id: 1,
            provider,
            start_block: 100,
            amb_proxies: vec![AmbContractConfig {
                address: Address::from([1; 20]),
                version: 6,
                started_at_block: 100,
                abi: Some(xdai_foreign_abi),
            }],
            mediators: vec![],
        }];

        let err = AbiRegistry::from_chains(&chains).expect_err("xDai ABI must be rejected as AMB");
        assert!(
            err.to_string().contains("topic0"),
            "unexpected error: {err}"
        );
    }
}
