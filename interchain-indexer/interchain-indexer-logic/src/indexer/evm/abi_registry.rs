// SPDX-License-Identifier: LicenseRef-Blockscout

//! Protocol-agnostic half of the ADR-006 contract version registry.
//!
//! Everything here is generic over a protocol-supplied `Kind` (e.g. AMB's
//! `ContractKind`) and never keys on an event *name* — only on `topic0` and
//! block number. Protocol-specific concerns (side inference, dispatch on
//! event name, the AMB `chain_by_side` map) stay in each protocol's own
//! `abi.rs`.

use std::collections::{HashMap, HashSet};

use alloy::{
    json_abi::{Event, JsonAbi},
    primitives::{Address, B256},
    rpc::types::Filter,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

/// One deployed implementation behind an address, valid from `started_at_block`
/// until the next version's `started_at_block` (the last one is open-ended).
#[derive(Clone, Debug)]
pub(crate) struct ContractVersion<K> {
    pub(crate) started_at_block: u64,
    pub(crate) kind: K,
    pub(crate) events_by_topic: HashMap<B256, Event>,
}

/// Every configured version of one `(chain_id, address)`, ordered by
/// `started_at_block` ascending.
///
/// A list rather than a single entry because a proxy is upgraded **behind
/// the same address**: `bridge_contracts`' unique key is
/// `(bridge_id, chain_id, address, version)` and its `started_at_block` column
/// is documented as "needed to select proper contract for the concrete block".
/// Address alone therefore cannot identify the implementation — only
/// `(address, block)` can.
#[derive(Clone, Debug)]
pub(crate) struct ContractAbi<K> {
    pub(crate) chain_id: i64,
    pub(crate) address: Address,
    pub(crate) versions: Vec<ContractVersion<K>>,
}

impl<K> ContractAbi<K> {
    /// The version in force at `block_number`, or `None` for a block below the
    /// earliest configured version.
    fn version_at(&self, block_number: u64) -> Option<&ContractVersion<K>> {
        self.versions
            .iter()
            .rev()
            .find(|version| version.started_at_block <= block_number)
    }

    fn declares_topic(&self, topic: &B256) -> bool {
        self.versions
            .iter()
            .any(|version| version.events_by_topic.contains_key(topic))
    }
}

/// What a fetched log resolved to. The filter matches any configured address
/// crossed with any configured topic, so a log coming back is not by itself
/// evidence that it belongs to a contract that declares it.
#[derive(Debug)]
pub(crate) enum LogResolution<'a, K> {
    /// The version in force at this block declares this topic.
    Matched(&'a Event, K),
    /// Not ours: an unconfigured address, or a topic this address never
    /// declares in any version. Ordinary, and expected from the cross product.
    NotConfigured,
    /// The address declares this topic — in a *different* version window than
    /// the block falls into. The log is dropped, which is correct only if the
    /// config is right, so it is counted rather than passed over in silence.
    WrongVersion,
}

#[derive(Clone, Debug)]
pub(crate) struct AbiRegistry<K> {
    pub(crate) contracts: HashMap<(i64, Address), ContractAbi<K>>,
}

// Not derived: `#[derive(Default)]` would add a spurious `K: Default` bound
// even though the only field, a `HashMap`, needs none.
impl<K> Default for AbiRegistry<K> {
    fn default() -> Self {
        Self {
            contracts: HashMap::new(),
        }
    }
}

impl<K: Copy> AbiRegistry<K> {
    #[cfg(test)]
    pub(crate) fn from_contracts_for_test(contracts: Vec<ContractAbi<K>>) -> Self {
        Self {
            contracts: contracts
                .into_iter()
                .map(|contract| ((contract.chain_id, contract.address), contract))
                .collect(),
        }
    }

    pub(crate) fn insert_contract(
        &mut self,
        chain_id: i64,
        address: Address,
        started_at_block: u64,
        kind: K,
        abi_value: Option<&Value>,
        required_events: &[&str],
    ) -> Result<()> {
        let abi_value = abi_value.with_context(|| {
            format!("missing ABI for contract row chain_id={chain_id} address={address}")
        })?;
        let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
            format!("invalid ABI for contract row chain_id={chain_id} address={address}")
        })?;

        let mut events_by_topic = HashMap::new();
        for event_name in required_events {
            let event = abi
                .events
                .get(*event_name)
                .and_then(|events| events.first())
                .cloned()
                .with_context(|| {
                    format!(
                        "ABI for chain_id={chain_id} address={address} missing event {event_name}"
                    )
                })?;
            events_by_topic.insert(event.selector(), event);
        }

        ensure!(
            !events_by_topic.is_empty(),
            "ABI for chain_id={chain_id} address={address} has no subscribed items"
        );

        let entry = self
            .contracts
            .entry((chain_id, address))
            .or_insert_with(|| ContractAbi {
                chain_id,
                address,
                versions: Vec::new(),
            });

        // Distinct floors are what makes `(address, block)` resolvable at all;
        // two versions claiming the same block have no defined order, and
        // "whichever was inserted last" is not a contract worth having.
        ensure!(
            entry
                .versions
                .iter()
                .all(|version| version.started_at_block != started_at_block),
            "contract chain_id={chain_id} address={address} has two versions \
             starting at block {started_at_block}"
        );

        entry.versions.push(ContractVersion {
            started_at_block,
            kind,
            events_by_topic,
        });
        entry
            .versions
            .sort_by_key(|version| version.started_at_block);

        Ok(())
    }

    /// Resolves a fetched log against the version in force at its block.
    ///
    /// Resolution is by `(address, block)`, not by address: an upgraded proxy
    /// keeps its address, and `topic0` covers the event signature but **not**
    /// which parameters are `indexed`, so two versions can share a topic and
    /// still decode differently.
    pub(crate) fn resolve_log(
        &self,
        chain_id: i64,
        address: Address,
        topic: &B256,
        block_number: u64,
    ) -> LogResolution<'_, K> {
        let Some(contract) = self.contracts.get(&(chain_id, address)) else {
            return LogResolution::NotConfigured;
        };

        match contract.version_at(block_number) {
            Some(version) => match version.events_by_topic.get(topic) {
                Some(event) => LogResolution::Matched(event, version.kind),
                None if contract.declares_topic(topic) => LogResolution::WrongVersion,
                None => LogResolution::NotConfigured,
            },
            // Below every configured version: the contract did not exist yet
            // as far as this config is concerned.
            None if contract.declares_topic(topic) => LogResolution::WrongVersion,
            None => LogResolution::NotConfigured,
        }
    }

    /// `resolve_log` reduced to the match case, for call sites that only need
    /// "is this one of ours". [`LogResolution::WrongVersion`] is reported by
    /// each protocol's dispatcher, which sees every log once; reporting it
    /// here too would count the same log several times.
    pub(crate) fn event_for_log(
        &self,
        chain_id: i64,
        address: Address,
        topic: &B256,
        block_number: u64,
    ) -> Option<(&Event, K)> {
        match self.resolve_log(chain_id, address, topic, block_number) {
            LogResolution::Matched(event, kind) => Some((event, kind)),
            _ => None,
        }
    }

    /// Union across every address and every version on the chain.
    ///
    /// Deliberately wider than any single version window: narrowing the *fetch*
    /// per version would have to be mirrored exactly by the retry path, and a
    /// replay whose filter is narrower than the forward scan resolves holes it
    /// never re-read. Version selection belongs at decode time, where it costs
    /// one lookup and no cross-path invariant.
    pub(crate) fn filter_for_chain(&self, chain_id: i64) -> Result<Filter> {
        let mut addresses = HashSet::new();
        let mut topics = HashSet::new();
        for contract in self
            .contracts
            .values()
            .filter(|contract| contract.chain_id == chain_id)
        {
            addresses.insert(contract.address);
            for version in &contract.versions {
                topics.extend(version.events_by_topic.keys().copied());
            }
        }
        if addresses.is_empty() || topics.is_empty() {
            bail!("no ABI filter entries for chain {chain_id}");
        }

        Ok(Filter::new()
            .address(addresses.into_iter().collect::<Vec<_>>())
            .event_signature(topics.into_iter().collect::<Vec<_>>()))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy::{
        json_abi::Event,
        primitives::{Address, B256, keccak256},
    };

    use super::{AbiRegistry, ContractAbi, ContractVersion, LogResolution};

    #[test]
    fn filter_for_chain_uses_precomputed_topic0_values_directly() {
        let topic = B256::from([1; 32]);
        let rehashed_topic = B256::from(keccak256(topic.as_slice()));
        let address = Address::from([2; 20]);
        let mut events_by_topic = HashMap::new();
        events_by_topic.insert(
            topic,
            Event {
                name: "UserRequestForAffirmation".into(),
                inputs: Vec::new(),
                anonymous: false,
            },
        );

        let registry = AbiRegistry {
            contracts: HashMap::from([(
                (1, address),
                ContractAbi {
                    chain_id: 1,
                    address,
                    versions: vec![ContractVersion {
                        started_at_block: 0,
                        kind: (),
                        events_by_topic,
                    }],
                },
            )]),
        };

        let filter = registry.filter_for_chain(1).expect("filter");

        assert!(filter.topics[0].contains(&topic));
        assert!(!filter.topics[0].contains(&rehashed_topic));
    }

    fn event_abi(name: &str) -> serde_json::Value {
        serde_json::json!([{"type":"event","name":name,"inputs":[],"anonymous":false}])
    }

    fn topic_of(name: &str) -> B256 {
        Event {
            name: name.into(),
            inputs: Vec::new(),
            anonymous: false,
        }
        .selector()
    }

    fn registry_with_versions(address: Address, versions: &[(u64, &str)]) -> AbiRegistry<()> {
        let mut registry = AbiRegistry::default();
        for (started_at_block, event_name) in versions {
            registry
                .insert_contract(
                    1,
                    address,
                    *started_at_block,
                    (),
                    Some(&event_abi(event_name)),
                    &[event_name],
                )
                .expect("version inserted");
        }
        registry
    }

    /// Two versions of one address are both kept: the address alone cannot
    /// identify an upgraded proxy's implementation, only `(address, block)`.
    #[test]
    fn insert_contract_keeps_every_version_of_one_address_ordered_by_start_block() {
        let address = Address::from([2; 20]);
        // Inserted out of order on purpose.
        let registry =
            registry_with_versions(address, &[(1_000, "SecondEvent"), (0, "FirstEvent")]);

        let contract = registry.contracts.get(&(1, address)).expect("contract");
        assert_eq!(
            contract
                .versions
                .iter()
                .map(|version| version.started_at_block)
                .collect::<Vec<_>>(),
            vec![0, 1_000]
        );
    }

    #[test]
    fn insert_contract_rejects_two_versions_starting_at_the_same_block() {
        let address = Address::from([2; 20]);
        let mut registry = registry_with_versions(address, &[(500, "FirstEvent")]);

        let err = registry
            .insert_contract(
                1,
                address,
                500,
                (),
                Some(&event_abi("DuplicateEvent")),
                &["DuplicateEvent"],
            )
            .expect_err("ambiguous version boundary rejected");

        assert!(
            err.to_string()
                .contains("two versions starting at block 500"),
            "unexpected error: {err}"
        );
    }

    /// The core of version resolution: the same address decodes against
    /// different ABIs depending on the block, and `topic0` cannot disambiguate
    /// because it covers the signature but not which parameters are `indexed`.
    #[test]
    fn resolve_log_selects_the_version_in_force_at_the_block() {
        let address = Address::from([2; 20]);
        let registry =
            registry_with_versions(address, &[(0, "FirstEvent"), (1_000, "SecondEvent")]);
        let first = topic_of("FirstEvent");
        let second = topic_of("SecondEvent");

        assert!(matches!(
            registry.resolve_log(1, address, &first, 999),
            LogResolution::Matched(event, _) if event.name == "FirstEvent"
        ));
        assert!(matches!(
            registry.resolve_log(1, address, &second, 1_000),
            LogResolution::Matched(event, _) if event.name == "SecondEvent"
        ));
    }

    /// A topic the address declares, but not in the version active at this
    /// block. Distinguished from ordinary noise because it is the shape a
    /// wrong `started_at_block` takes: real events silently discarded.
    #[test]
    fn resolve_log_reports_a_topic_from_another_version_as_wrong_version() {
        let address = Address::from([2; 20]);
        let registry =
            registry_with_versions(address, &[(0, "FirstEvent"), (1_000, "SecondEvent")]);

        assert!(matches!(
            registry.resolve_log(1, address, &topic_of("SecondEvent"), 999),
            LogResolution::WrongVersion
        ));
        assert!(matches!(
            registry.resolve_log(1, address, &topic_of("FirstEvent"), 1_000),
            LogResolution::WrongVersion
        ));
    }

    /// A block below every configured version, for a topic this address does
    /// declare: the config claims the contract did not exist yet, so a log
    /// from it means a `started_at_block` is too high.
    #[test]
    fn resolve_log_reports_a_block_below_every_version_as_wrong_version() {
        let address = Address::from([2; 20]);
        let registry = registry_with_versions(address, &[(1_000, "FirstEvent")]);

        assert!(matches!(
            registry.resolve_log(1, address, &topic_of("FirstEvent"), 999),
            LogResolution::WrongVersion
        ));
    }

    /// Unknown address, and a topic no version of a known address declares.
    /// Both are expected: the filter matches any configured address crossed
    /// with any configured topic, so cross-matches come back routinely and
    /// must not be counted as a misconfiguration.
    #[test]
    fn resolve_log_reports_unknown_address_or_topic_as_not_configured() {
        let address = Address::from([2; 20]);
        let registry = registry_with_versions(address, &[(0, "FirstEvent")]);

        assert!(matches!(
            registry.resolve_log(1, Address::from([9; 20]), &topic_of("FirstEvent"), 10),
            LogResolution::NotConfigured
        ));
        assert!(matches!(
            registry.resolve_log(1, address, &topic_of("UnrelatedEvent"), 10),
            LogResolution::NotConfigured
        ));
    }

    /// The fetch filter must union every version's topics. Narrowing it per
    /// version would have to be mirrored exactly by the retry path, and a
    /// replay narrower than the forward scan resolves holes it never re-read.
    #[test]
    fn filter_for_chain_unions_topics_across_versions() {
        let address = Address::from([2; 20]);
        let registry =
            registry_with_versions(address, &[(0, "FirstEvent"), (1_000, "SecondEvent")]);

        let filter = registry.filter_for_chain(1).expect("filter");

        assert!(filter.topics[0].contains(&topic_of("FirstEvent")));
        assert!(filter.topics[0].contains(&topic_of("SecondEvent")));
    }
}
