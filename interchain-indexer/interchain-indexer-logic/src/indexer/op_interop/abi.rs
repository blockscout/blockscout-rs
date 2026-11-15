// SPDX-License-Identifier: LicenseRef-Blockscout

use std::collections::HashSet;

use alloy::{
    primitives::{Address, B256, ChainId},
    rpc::types::Filter,
};
use anyhow::{Context, Result, ensure};

use crate::indexer::evm::abi_registry;

use super::{
    indexer::{OpInteropChainConfig, OpInteropContractConfig},
    version::{OpInteropContractKind, grammar_for, known_kinds},
};

/// Typed declarations of grammar revision 1 (the Lagoon launch set), used for
/// decoding. Their signature hashes are checked against the pinned `topic0`
/// literals of `version.rs` by a test; they are never the source of a
/// `topic0`.
pub(crate) mod v1 {
    alloy::sol! {
        struct Identifier {
            address origin;
            uint256 blockNumber;
            uint256 logIndex;
            uint256 timestamp;
            uint256 chainId;
        }

        event ExecutingMessage(bytes32 indexed msgHash, Identifier id);

        event SentMessage(
            uint256 indexed destination,
            address indexed target,
            uint256 indexed messageNonce,
            address sender,
            bytes message
        );

        event RelayedMessage(
            uint256 indexed source,
            uint256 indexed messageNonce,
            bytes32 indexed messageHash,
            bytes32 returnDataHash
        );
    }
}

/// What a configured contract window is: which predeploy, decoded with which
/// grammar revision.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContractKind {
    pub(crate) contract: OpInteropContractKind,
    pub(crate) grammar_version: i16,
}

pub(crate) type LogResolution<'a> = abi_registry::LogResolution<'a, ContractKind>;

/// Thin OP interop wrapper around the protocol-agnostic
/// [`abi_registry::AbiRegistry`] (ADR-006: a contract version is
/// `(address, block)`). The events come from the grammar table in
/// `version.rs`, not from a JSON ABI in the config.
#[derive(Clone, Debug, Default)]
pub(crate) struct OpInteropRegistry {
    inner: abi_registry::AbiRegistry<ContractKind>,
}

impl OpInteropRegistry {
    /// Builds the registry and, with it, validates every contract entry of
    /// the bridge. Every error here is a config mistake and stops startup.
    pub(crate) fn from_chains(chains: &[OpInteropChainConfig]) -> Result<Self> {
        Self::from_chain_contracts(
            chains
                .iter()
                .map(|chain| (chain.chain_id, chain.contracts.as_slice())),
        )
    }

    pub(crate) fn from_chain_contracts<'a>(
        chains: impl IntoIterator<Item = (i64, &'a [OpInteropContractConfig])>,
    ) -> Result<Self> {
        let mut registry = Self::default();
        for (chain_id, contracts) in chains {
            registry.insert_chain(chain_id, contracts)?;
        }
        Ok(registry)
    }

    fn insert_chain(&mut self, chain_id: i64, contracts: &[OpInteropContractConfig]) -> Result<()> {
        let mut configured_kinds = HashSet::new();

        for contract in contracts {
            let raw_kind = contract.kind.as_deref().with_context(|| {
                format!(
                    "OP interop contract {} on chain {chain_id} has no `kind`; known kinds: {}",
                    contract.address,
                    known_kinds()
                )
            })?;
            let kind: OpInteropContractKind = raw_kind.parse().with_context(|| {
                format!(
                    "invalid `kind` for OP interop contract {} on chain {chain_id}",
                    contract.address
                )
            })?;

            // A wrong address would otherwise index nothing, silently.
            ensure!(
                contract.address == kind.predeploy_address(),
                "OP interop contract of kind {} on chain {chain_id} is configured at {}, \
                 but that kind is the predeploy {}",
                kind.as_str(),
                contract.address,
                kind.predeploy_address()
            );

            let grammar = grammar_for(kind, contract.version).with_context(|| {
                format!(
                    "invalid `version` for OP interop contract {} (kind {}) on chain {chain_id}",
                    contract.address,
                    kind.as_str()
                )
            })?;
            let events = grammar
                .events
                .iter()
                .map(|event| event.json_abi_event())
                .collect::<Result<Vec<_>>>()?;

            self.inner
                .insert_contract_events(
                    chain_id,
                    contract.address,
                    contract.started_at_block,
                    ContractKind {
                        contract: kind,
                        grammar_version: grammar.version,
                    },
                    events,
                )
                .with_context(|| {
                    format!(
                        "failed to register OP interop contract {} (kind {}) on chain {chain_id}",
                        contract.address,
                        kind.as_str()
                    )
                })?;
            configured_kinds.insert(kind);

            tracing::debug!(
                chain_id,
                address = %contract.address,
                kind = kind.as_str(),
                grammar_version = grammar.version,
                started_at_block = contract.started_at_block,
                "registered OP interop contract version"
            );
        }

        // A relay is `ExecutingMessage` (inbox) + `RelayedMessage` (messenger)
        // and a send is a messenger event: a chain with one kind missing
        // cannot be indexed correctly.
        for kind in OpInteropContractKind::ALL {
            ensure!(
                configured_kinds.contains(&kind),
                "OP interop chain {chain_id} has no contract of kind {}; every chain needs \
                 one entry of each kind: {}",
                kind.as_str(),
                known_kinds()
            );
        }

        Ok(())
    }

    /// Resolves a fetched log against the version window in force at its
    /// block. A chain id outside the storage range cannot be configured.
    pub(crate) fn resolve_log(
        &self,
        chain_id: ChainId,
        address: Address,
        topic: &B256,
        block_number: u64,
    ) -> LogResolution<'_> {
        match i64::try_from(chain_id) {
            Ok(chain_id) => self
                .inner
                .resolve_log(chain_id, address, topic, block_number),
            Err(_) => LogResolution::NotConfigured,
        }
    }

    /// The one filter of a chain: its configured addresses crossed with every
    /// topic they declare. Used by the forward streams and by the retry path
    /// alike, so a replay is never narrower than the scan.
    pub(crate) fn filter_for_chain(&self, chain_id: i64) -> Result<Filter> {
        self.inner.filter_for_chain(chain_id)
    }
}

#[cfg(test)]
mod tests {
    use alloy::{primitives::address, sol_types::SolEvent};

    use super::{
        super::{
            test_fixtures::{
                DESTINATION_CHAIN_ID, SOURCE_CHAIN_ID, executing_message_log, relayed_message_log,
                sent_message_logs,
            },
            version::{
                CROSS_L2_INBOX, EXECUTING_MESSAGE_TOPIC0, L2_TO_L2_MESSENGER,
                RELAYED_MESSAGE_TOPIC0, SENT_MESSAGE_TOPIC0,
            },
        },
        *,
    };

    fn contract(address: Address, kind: &str, version: i16, block: u64) -> OpInteropContractConfig {
        OpInteropContractConfig {
            address,
            kind: Some(kind.to_string()),
            version,
            started_at_block: block,
        }
    }

    fn valid_contracts() -> Vec<OpInteropContractConfig> {
        vec![
            contract(CROSS_L2_INBOX, "cross_l2_inbox", 1, 465_679),
            contract(L2_TO_L2_MESSENGER, "l2_to_l2_messenger", 1, 465_679),
        ]
    }

    fn registry_error(contracts: Vec<OpInteropContractConfig>) -> String {
        let err = OpInteropRegistry::from_chain_contracts([(420_130_019, contracts.as_slice())])
            .expect_err("config rejected");
        format!("{err:#}")
    }

    /// The literals are what the deployed contracts emit.
    #[test]
    fn test_pinned_topic0_matches_fixture_logs() {
        for sent in sent_message_logs() {
            assert_eq!(sent.topic0(), Some(&SENT_MESSAGE_TOPIC0));
            assert_eq!(sent.address(), L2_TO_L2_MESSENGER);
        }
        let executing = executing_message_log();
        assert_eq!(executing.topic0(), Some(&EXECUTING_MESSAGE_TOPIC0));
        assert_eq!(executing.address(), CROSS_L2_INBOX);
        let relayed = relayed_message_log();
        assert_eq!(relayed.topic0(), Some(&RELAYED_MESSAGE_TOPIC0));
        assert_eq!(relayed.address(), L2_TO_L2_MESSENGER);
    }

    /// The typed declarations used for decoding agree with the literals.
    #[test]
    fn test_sol_signature_hashes_match_pinned_topic0() {
        assert_eq!(
            v1::ExecutingMessage::SIGNATURE_HASH,
            EXECUTING_MESSAGE_TOPIC0
        );
        assert_eq!(v1::SentMessage::SIGNATURE_HASH, SENT_MESSAGE_TOPIC0);
        assert_eq!(v1::RelayedMessage::SIGNATURE_HASH, RELAYED_MESSAGE_TOPIC0);
        assert_eq!(
            v1::ExecutingMessage::SIGNATURE,
            "ExecutingMessage(bytes32,(address,uint256,uint256,uint256,uint256))"
        );
        assert_eq!(
            v1::SentMessage::SIGNATURE,
            "SentMessage(uint256,address,uint256,address,bytes)"
        );
        assert_eq!(
            v1::RelayedMessage::SIGNATURE,
            "RelayedMessage(uint256,uint256,bytes32,bytes32)"
        );
    }

    #[test]
    fn test_registry_rejects_unknown_kind() {
        for unknown in ["superchain_eth_bridge", "l2_to_l2_mesenger"] {
            let mut contracts = valid_contracts();
            contracts[1].kind = Some(unknown.to_string());
            let message = registry_error(contracts);
            assert!(
                message.contains("unknown OP interop contract kind")
                    && message.contains("cross_l2_inbox, l2_to_l2_messenger")
                    && message.contains("420130019"),
                "unexpected error: {message}"
            );
        }

        let mut contracts = valid_contracts();
        contracts[0].kind = None;
        let message = registry_error(contracts);
        assert!(
            message.contains("has no `kind`"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn test_registry_rejects_non_predeploy_address() {
        let mut contracts = valid_contracts();
        contracts[1].address = address!("4200000000000000000000000000000000000024");
        let message = registry_error(contracts);
        assert!(
            message.contains("l2_to_l2_messenger")
                && message.contains("0x4200000000000000000000000000000000000024")
                && message.contains("0x4200000000000000000000000000000000000023"),
            "unexpected error: {message}"
        );

        // The two predeploys swapped: each address is valid, for the other kind.
        let swapped = vec![
            contract(L2_TO_L2_MESSENGER, "cross_l2_inbox", 1, 1),
            contract(CROSS_L2_INBOX, "l2_to_l2_messenger", 1, 1),
        ];
        assert!(registry_error(swapped).contains("is the predeploy"));
    }

    #[test]
    fn test_registry_rejects_unknown_grammar_version() {
        let mut contracts = valid_contracts();
        contracts[0].version = 2;
        let message = registry_error(contracts);
        assert!(
            message.contains("unknown OP interop grammar version 2")
                && message.contains("known versions: 1"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn test_registry_requires_both_kinds_per_chain() {
        for missing in [0, 1] {
            let mut contracts = valid_contracts();
            let removed = contracts.remove(missing);
            let message = registry_error(contracts);
            assert!(
                message.contains("has no contract of kind")
                    && message.contains(removed.kind.as_deref().unwrap()),
                "unexpected error: {message}"
            );
        }
        assert!(registry_error(Vec::new()).contains("has no contract of kind"));

        // Both kinds on one chain do not cover another chain.
        let complete = valid_contracts();
        let inbox_only = vec![contract(CROSS_L2_INBOX, "cross_l2_inbox", 1, 1)];
        let err = OpInteropRegistry::from_chain_contracts([
            (420_130_019, complete.as_slice()),
            (420_130_020, inbox_only.as_slice()),
        ])
        .expect_err("second chain is incomplete");
        assert!(format!("{err:#}").contains("420130020"));
    }

    /// Two windows of one address cannot start at the same block (ADR-006);
    /// the shared registry enforces it for code-declared events too.
    #[test]
    fn test_registry_rejects_two_versions_at_one_block() {
        let mut contracts = valid_contracts();
        contracts.push(contract(CROSS_L2_INBOX, "cross_l2_inbox", 1, 465_679));
        assert!(registry_error(contracts).contains("two versions starting at block 465679"));
    }

    #[test]
    fn test_registry_filter_covers_both_addresses_and_three_topics() {
        let source = valid_contracts();
        let destination = vec![
            contract(CROSS_L2_INBOX, "cross_l2_inbox", 1, 829_213),
            contract(L2_TO_L2_MESSENGER, "l2_to_l2_messenger", 1, 829_213),
        ];
        let registry = OpInteropRegistry::from_chain_contracts([
            (SOURCE_CHAIN_ID as i64, source.as_slice()),
            (DESTINATION_CHAIN_ID as i64, destination.as_slice()),
        ])
        .expect("valid config");

        for chain_id in [SOURCE_CHAIN_ID, DESTINATION_CHAIN_ID] {
            let filter = registry
                .filter_for_chain(chain_id as i64)
                .expect("filter for a configured chain");
            assert_eq!(filter.address.len(), 2);
            assert!(filter.address.contains(&CROSS_L2_INBOX));
            assert!(filter.address.contains(&L2_TO_L2_MESSENGER));
            assert_eq!(filter.topics[0].len(), 3);
            for topic in [
                EXECUTING_MESSAGE_TOPIC0,
                SENT_MESSAGE_TOPIC0,
                RELAYED_MESSAGE_TOPIC0,
            ] {
                assert!(filter.topics[0].contains(&topic));
            }
        }
        assert!(registry.filter_for_chain(1).is_err());

        // Resolution is by `(address, block)`: the right contract at or above
        // its floor matches; the cross product of the filter does not.
        assert!(matches!(
            registry.resolve_log(SOURCE_CHAIN_ID, L2_TO_L2_MESSENGER, &SENT_MESSAGE_TOPIC0, 465_679),
            LogResolution::Matched(event, kind)
                if event.name == "SentMessage"
                    && kind.contract == OpInteropContractKind::L2ToL2Messenger
                    && kind.grammar_version == 1
        ));
        assert!(matches!(
            registry.resolve_log(
                SOURCE_CHAIN_ID,
                CROSS_L2_INBOX,
                &SENT_MESSAGE_TOPIC0,
                465_679
            ),
            LogResolution::NotConfigured
        ));
        assert!(matches!(
            registry.resolve_log(
                SOURCE_CHAIN_ID,
                L2_TO_L2_MESSENGER,
                &SENT_MESSAGE_TOPIC0,
                465_678
            ),
            LogResolution::WrongVersion
        ));
        assert!(matches!(
            registry.resolve_log(u64::MAX, L2_TO_L2_MESSENGER, &SENT_MESSAGE_TOPIC0, 465_679),
            LogResolution::NotConfigured
        ));
    }
}
