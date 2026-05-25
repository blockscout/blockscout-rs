use std::collections::{HashMap, HashSet};

use alloy::{
    json_abi::{Event, Function, JsonAbi},
    primitives::{Address, B256, Selector},
    rpc::types::Filter,
};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

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

#[derive(Clone, Debug)]
pub(crate) struct ContractAbi {
    pub(crate) chain_id: i64,
    pub(crate) address: Address,
    pub(crate) kind: ContractKind,
    pub(crate) events_by_topic: HashMap<B256, Event>,
    pub(crate) functions_by_selector: HashMap<Selector, Function>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AbiRegistry {
    contracts: HashMap<(i64, Address), ContractAbi>,
    events_by_chain_topic: HashMap<(i64, B256), (Address, Event, ContractKind)>,
    mediator_by_chain: HashMap<i64, Address>,
    chain_by_side: HashMap<AmbSide, i64>,
}

impl AbiRegistry {
    #[cfg(test)]
    pub(crate) fn from_contracts_for_test(contracts: Vec<ContractAbi>) -> Self {
        Self {
            contracts: contracts
                .into_iter()
                .map(|contract| ((contract.chain_id, contract.address), contract))
                .collect(),
            events_by_chain_topic: HashMap::new(),
            mediator_by_chain: HashMap::new(),
            chain_by_side: HashMap::new(),
        }
    }

    pub(crate) fn from_chains(chains: &[AmbChainConfig]) -> Result<Self> {
        let mut registry = Self::default();

        for chain in chains {
            let amb_grammar = amb_grammar_for(chain.amb_version)?;
            let _amb_version = amb_grammar.version;
            let side = amb_side_for_abi(
                chain.chain_id,
                chain.amb_proxy_address,
                chain.amb_abi.as_ref(),
                amb_grammar,
            )?;
            ensure!(
                registry
                    .chain_by_side
                    .insert(side, chain.chain_id)
                    .is_none(),
                "AMB bridge config has multiple {side:?} chains"
            );
            let amb_events = match side {
                AmbSide::Foreign => amb_grammar.foreign_events,
                AmbSide::Home => amb_grammar.home_events,
            };
            registry.insert_contract(
                chain.chain_id,
                chain.amb_proxy_address,
                ContractKind::AmbProxy {
                    side,
                    header_layout: amb_grammar.header_layout,
                },
                chain.amb_abi.as_ref(),
                amb_events,
                &[],
            )?;

            let mediator_grammar = mediator_grammar_for(chain.mediator_version)?;
            let _mediator_version = mediator_grammar.version;
            registry.insert_contract(
                chain.chain_id,
                chain.mediator_address,
                ContractKind::OmnibridgeMediator,
                chain.mediator_abi.as_ref(),
                mediator_grammar.events,
                mediator_grammar.functions,
            )?;
            registry
                .mediator_by_chain
                .insert(chain.chain_id, chain.mediator_address);
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

    #[allow(clippy::too_many_arguments)]
    fn insert_contract(
        &mut self,
        chain_id: i64,
        address: Address,
        kind: ContractKind,
        abi_value: Option<&Value>,
        required_events: &[&str],
        required_functions: &[&str],
    ) -> Result<()> {
        let abi_value = abi_value.with_context(|| {
            format!("missing ABI for AMB contract row chain_id={chain_id} address={address}")
        })?;
        let abi: JsonAbi = serde_json::from_value(abi_value.clone()).with_context(|| {
            format!("invalid ABI for AMB contract row chain_id={chain_id} address={address}")
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
            //tracing::warn!("Inserting event for chain {}: {} -> topic0 {}", chain_id, event_name, event.selector().to_hex());
            events_by_topic.insert(event.selector(), event);
        }

        let mut functions_by_selector = HashMap::new();
        for function_name in required_functions {
            let function = abi
                .functions
                .get(*function_name)
                .and_then(|functions| functions.first())
                .cloned()
                .with_context(|| {
                    format!(
                        "ABI for chain_id={chain_id} address={address} missing function {function_name}"
                    )
                })?;
            functions_by_selector.insert(function.selector(), function);
        }

        ensure!(
            !events_by_topic.is_empty() || !functions_by_selector.is_empty(),
            "ABI for chain_id={chain_id} address={address} has no subscribed items"
        );

        for (topic, event) in &events_by_topic {
            self.events_by_chain_topic
                .insert((chain_id, *topic), (address, event.clone(), kind));
        }
        self.contracts.insert(
            (chain_id, address),
            ContractAbi {
                chain_id,
                address,
                kind,
                events_by_topic,
                functions_by_selector,
            },
        );

        Ok(())
    }

    pub(crate) fn event_for_log(
        &self,
        chain_id: i64,
        address: Address,
        topic: &B256,
    ) -> Option<(&Event, ContractKind)> {
        self.contracts
            .get(&(chain_id, address))
            .and_then(|contract| {
                contract
                    .events_by_topic
                    .get(topic)
                    .map(|event| (event, contract.kind))
            })
    }

    pub(crate) fn function_for_selector(
        &self,
        chain_id: i64,
        mediator: Address,
        selector: Selector,
    ) -> Option<&Function> {
        self.contracts
            .get(&(chain_id, mediator))
            .and_then(|contract| contract.functions_by_selector.get(&selector))
    }

    pub(crate) fn filter_for_chain(&self, chain_id: i64) -> Result<Filter> {
        let mut addresses = HashSet::new();
        let mut topics = HashSet::new();
        for contract in self
            .contracts
            .values()
            .filter(|contract| contract.chain_id == chain_id)
        {
            addresses.insert(contract.address);
            topics.extend(contract.events_by_topic.keys().copied());
        }
        if addresses.is_empty() || topics.is_empty() {
            bail!("no AMB ABI filter entries for chain {chain_id}");
        }

        Ok(Filter::new()
            .address(addresses.into_iter().collect::<Vec<_>>())
            .event_signature(topics.into_iter().collect::<Vec<_>>()))
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use alloy::{
        json_abi::Event,
        primitives::{Address, B256, keccak256},
    };

    use crate::indexer::amb::version::{AmbSide, amb_grammar_for};

    use super::{AbiRegistry, ContractAbi, ContractKind, amb_side_for_abi};

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
                    kind: ContractKind::OmnibridgeMediator,
                    events_by_topic,
                    functions_by_selector: HashMap::new(),
                },
            )]),
            events_by_chain_topic: HashMap::new(),
            mediator_by_chain: HashMap::new(),
            chain_by_side: HashMap::new(),
        };

        let filter = registry.filter_for_chain(1).expect("filter");

        assert!(filter.topics[0].contains(&topic));
        assert!(!filter.topics[0].contains(&rehashed_topic));
    }

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
}
