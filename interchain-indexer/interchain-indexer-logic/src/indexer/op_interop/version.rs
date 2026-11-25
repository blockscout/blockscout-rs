// SPDX-License-Identifier: LicenseRef-Blockscout

//! The OP interop event grammar, pinned in code.
//!
//! `bridges.json` carries no ABI for this protocol: a contract entry names a
//! `kind` and a grammar `version`, and everything else comes from here.

use std::str::FromStr;

use alloy::{
    json_abi::Event,
    primitives::{Address, B256, address, b256},
};
use anyhow::{Context, Result, bail, ensure};

/// `CrossL2Inbox` predeploy, the same address on every member chain.
pub(crate) const CROSS_L2_INBOX: Address = address!("4200000000000000000000000000000000000022");
/// `L2ToL2CrossDomainMessenger` predeploy, the same address on every member
/// chain.
pub(crate) const L2_TO_L2_MESSENGER: Address = address!("4200000000000000000000000000000000000023");

/// `SuperchainETHBridge` predeploy, the same address on every member chain.
pub(crate) const SUPERCHAIN_ETH_BRIDGE: Address =
    address!("4200000000000000000000000000000000000024");

/// The contract kinds this indexer reads; the `kind` of a `bridges.json`
/// contract entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OpInteropContractKind {
    CrossL2Inbox,
    L2ToL2Messenger,
    SuperchainEthBridge,
}

impl OpInteropContractKind {
    pub(crate) const ALL: [Self; 3] = [
        Self::CrossL2Inbox,
        Self::L2ToL2Messenger,
        Self::SuperchainEthBridge,
    ];

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::CrossL2Inbox => "cross_l2_inbox",
            Self::L2ToL2Messenger => "l2_to_l2_messenger",
            Self::SuperchainEthBridge => "superchain_eth_bridge",
        }
    }

    /// The only address a contract of this kind can have.
    pub(crate) fn predeploy_address(&self) -> Address {
        match self {
            Self::CrossL2Inbox => CROSS_L2_INBOX,
            Self::L2ToL2Messenger => L2_TO_L2_MESSENGER,
            Self::SuperchainEthBridge => SUPERCHAIN_ETH_BRIDGE,
        }
    }
}

impl FromStr for OpInteropContractKind {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match Self::ALL.into_iter().find(|kind| kind.as_str() == value) {
            Some(kind) => Ok(kind),
            None => bail!(
                "unknown OP interop contract kind {value:?}; known kinds: {}",
                known_kinds()
            ),
        }
    }
}

pub(crate) fn known_kinds() -> String {
    OpInteropContractKind::ALL
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

// `topic0` values, transcribed from logs emitted by the deployed contracts.
// They are literals on purpose: deriving them from the `sol!` declarations
// they are used to check would make the check circular.

/// `ExecutingMessage(bytes32,(address,uint256,uint256,uint256,uint256))`
pub(crate) const EXECUTING_MESSAGE_TOPIC0: B256 =
    b256!("5c37832d2e8d10e346e55ad62071a6a2f9fa5130614ef2ec6617555c6f467ba7");
/// `SentMessage(uint256,address,uint256,address,bytes)`
pub(crate) const SENT_MESSAGE_TOPIC0: B256 =
    b256!("382409ac69001e11931a28435afef442cbfd20d9891907e8fa373ba7d351f320");
/// `RelayedMessage(uint256,uint256,bytes32,bytes32)`
pub(crate) const RELAYED_MESSAGE_TOPIC0: B256 =
    b256!("c270d73e26d2d39dee7ef92093555927e344e243415547ecc350b2b5385b68a2");

/// `SendETH(address,address,uint256,uint256)`
pub(crate) const SEND_ETH_TOPIC0: B256 =
    b256!("ed98a2ff78833375c368471a747cdf0633024dde3f870feb08a934ac5be83402");
/// `RelayETH(address,address,uint256,uint256)`
pub(crate) const RELAY_ETH_TOPIC0: B256 =
    b256!("e5479bb8ebad3b9ac81f55f424a6289cf0a54ff2641708f41dcb2b26f264d359");
/// `relayETH(address,address,uint256)` selector.
pub(crate) const RELAY_ETH_SELECTOR: [u8; 4] = [0x4f, 0x0e, 0xdc, 0xc9];

/// The events this indexer reads. `as_str` is the `event` metric label. For
/// the messenger and inbox events it is also `op_interop_events.event_kind`;
/// `SendEth` and `RelayEth` are metric labels only and never reach the ledger.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::enum_variant_names)] // the names mirror the on-chain events
pub(crate) enum OpInteropEvent {
    ExecutingMessage,
    SentMessage,
    RelayedMessage,
    SendEth,
    RelayEth,
}

impl OpInteropEvent {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::ExecutingMessage => "executing_message",
            Self::SentMessage => "sent_message",
            Self::RelayedMessage => "relayed_message",
            Self::SendEth => "send_eth",
            Self::RelayEth => "relay_eth",
        }
    }
}

/// One event of a grammar: three representations of the same thing, tied
/// together when the registry is built (see [`GrammarEvent::json_abi_event`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct GrammarEvent {
    pub(crate) event: OpInteropEvent,
    /// Full signature, with parameter names and `indexed`, for
    /// `alloy::json_abi::Event::parse`. alloy's parser rejects named tuple
    /// components (in both the `(..)` and `tuple(..)` spellings), so the
    /// `Identifier` components are unnamed here; decoding uses the typed
    /// `sol!` declarations, and only the selector and the filter come from
    /// this entry.
    pub(crate) signature: &'static str,
    /// The pinned literal. The source of truth.
    pub(crate) topic0: B256,
}

impl GrammarEvent {
    /// The registry entry for this event. Fails when the signature string
    /// does not hash to the pinned `topic0`, so a typo in either cannot go
    /// unnoticed.
    pub(crate) fn json_abi_event(&self) -> Result<Event> {
        let event = Event::parse(self.signature).with_context(|| {
            format!(
                "failed to parse the pinned OP interop event signature {:?}",
                self.signature
            )
        })?;
        ensure!(
            event.selector() == self.topic0,
            "OP interop event {} hashes to {} but its pinned topic0 is {}",
            self.event.as_str(),
            event.selector(),
            self.topic0
        );
        Ok(event)
    }
}

/// What one grammar revision of one contract kind declares.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Grammar {
    pub(crate) kind: OpInteropContractKind,
    /// This service's grammar revision, not an on-chain counter.
    pub(crate) version: i16,
    pub(crate) events: &'static [GrammarEvent],
}

impl Grammar {
    pub(crate) fn event_for_topic(&self, topic0: &B256) -> Option<OpInteropEvent> {
        self.events
            .iter()
            .find(|event| event.topic0 == *topic0)
            .map(|event| event.event)
    }
}

/// Grammar revision 1, the Lagoon launch set: `CrossL2Inbox` 1.1.0.
static CROSS_L2_INBOX_V1: Grammar = Grammar {
    kind: OpInteropContractKind::CrossL2Inbox,
    version: 1,
    events: &[GrammarEvent {
        event: OpInteropEvent::ExecutingMessage,
        signature: "event ExecutingMessage(bytes32 indexed msgHash, (address,uint256,uint256,uint256,uint256) id)",
        topic0: EXECUTING_MESSAGE_TOPIC0,
    }],
};

/// Grammar revision 1, the Lagoon launch set: `L2ToL2CrossDomainMessenger`
/// 2.1.0.
static L2_TO_L2_MESSENGER_V1: Grammar = Grammar {
    kind: OpInteropContractKind::L2ToL2Messenger,
    version: 1,
    events: &[
        GrammarEvent {
            event: OpInteropEvent::SentMessage,
            signature: "event SentMessage(uint256 indexed destination, address indexed target, uint256 indexed messageNonce, address sender, bytes message)",
            topic0: SENT_MESSAGE_TOPIC0,
        },
        GrammarEvent {
            event: OpInteropEvent::RelayedMessage,
            signature: "event RelayedMessage(uint256 indexed source, uint256 indexed messageNonce, bytes32 indexed messageHash, bytes32 returnDataHash)",
            topic0: RELAYED_MESSAGE_TOPIC0,
        },
    ],
};

/// Grammar revision 1: `SuperchainETHBridge` 2.0.0. Both events are
/// diagnostics only; the transfer comes from the message payload.
static SUPERCHAIN_ETH_BRIDGE_V1: Grammar = Grammar {
    kind: OpInteropContractKind::SuperchainEthBridge,
    version: 1,
    events: &[
        GrammarEvent {
            event: OpInteropEvent::SendEth,
            signature: "event SendETH(address indexed from, address indexed to, uint256 amount, uint256 destination)",
            topic0: SEND_ETH_TOPIC0,
        },
        GrammarEvent {
            event: OpInteropEvent::RelayEth,
            signature: "event RelayETH(address indexed from, address indexed to, uint256 amount, uint256 source)",
            topic0: RELAY_ETH_TOPIC0,
        },
    ],
};

static GRAMMARS: [&Grammar; 3] = [
    &CROSS_L2_INBOX_V1,
    &L2_TO_L2_MESSENGER_V1,
    &SUPERCHAIN_ETH_BRIDGE_V1,
];

/// The grammar of `(kind, version)`. An unknown pair is a config error.
pub(crate) fn grammar_for(kind: OpInteropContractKind, version: i16) -> Result<&'static Grammar> {
    match GRAMMARS
        .iter()
        .copied()
        .find(|grammar| grammar.kind == kind && grammar.version == version)
    {
        Some(grammar) => Ok(grammar),
        None => {
            let known_versions = GRAMMARS
                .iter()
                .filter(|grammar| grammar.kind == kind)
                .map(|grammar| grammar.version.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "unknown OP interop grammar version {version} for contract kind {}; \
                 known versions: {known_versions}",
                kind.as_str()
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_kind_parses_known_kinds_and_rejects_the_rest() {
        assert_eq!(
            "cross_l2_inbox".parse::<OpInteropContractKind>().unwrap(),
            OpInteropContractKind::CrossL2Inbox
        );
        assert_eq!(
            "l2_to_l2_messenger"
                .parse::<OpInteropContractKind>()
                .unwrap(),
            OpInteropContractKind::L2ToL2Messenger
        );
        assert_eq!(
            OpInteropContractKind::CrossL2Inbox.predeploy_address(),
            CROSS_L2_INBOX
        );
        assert_eq!(
            OpInteropContractKind::L2ToL2Messenger.predeploy_address(),
            L2_TO_L2_MESSENGER
        );

        assert_eq!(
            "superchain_eth_bridge"
                .parse::<OpInteropContractKind>()
                .unwrap(),
            OpInteropContractKind::SuperchainEthBridge
        );
        assert_eq!(
            OpInteropContractKind::SuperchainEthBridge.predeploy_address(),
            SUPERCHAIN_ETH_BRIDGE
        );

        // Kinds of later slices are not accepted yet: accepting and ignoring
        // them would make a typo indistinguishable from an unused kind.
        for unknown in ["undelivered_message_exporter", "CrossL2Inbox", ""] {
            let err = unknown
                .parse::<OpInteropContractKind>()
                .expect_err("unknown kind rejected");
            let message = err.to_string();
            assert!(
                message.contains("cross_l2_inbox")
                    && message.contains("l2_to_l2_messenger")
                    && message.contains("superchain_eth_bridge"),
                "the error must list the known kinds: {message}"
            );
        }
    }

    #[test]
    fn test_grammar_for_knows_version_one_only() {
        for kind in OpInteropContractKind::ALL {
            let grammar = grammar_for(kind, 1).expect("version 1 exists");
            assert_eq!(grammar.kind, kind);
            assert_eq!(grammar.version, 1);

            let err = grammar_for(kind, 2).expect_err("version 2 does not exist");
            assert!(
                err.to_string().contains("known versions: 1"),
                "the error must name the known versions: {err}"
            );
        }
    }

    /// Every signature string parses and hashes to its pinned literal.
    #[test]
    fn test_grammar_signatures_hash_to_pinned_topic0() {
        for grammar in GRAMMARS {
            for grammar_event in grammar.events {
                let event = grammar_event
                    .json_abi_event()
                    .expect("signature parses and matches its topic0");
                assert_eq!(event.selector(), grammar_event.topic0);
                assert_eq!(
                    grammar.event_for_topic(&grammar_event.topic0),
                    Some(grammar_event.event)
                );
            }
        }
        assert_eq!(
            CROSS_L2_INBOX_V1.event_for_topic(&SENT_MESSAGE_TOPIC0),
            None,
            "the inbox does not declare a messenger event"
        );
        assert_eq!(
            SUPERCHAIN_ETH_BRIDGE_V1.event_for_topic(&SEND_ETH_TOPIC0),
            Some(OpInteropEvent::SendEth)
        );
        assert_eq!(
            SUPERCHAIN_ETH_BRIDGE_V1.event_for_topic(&RELAY_ETH_TOPIC0),
            Some(OpInteropEvent::RelayEth)
        );
        assert_eq!(
            L2_TO_L2_MESSENGER_V1.event_for_topic(&SEND_ETH_TOPIC0),
            None,
            "the messenger does not declare a bridge event"
        );
    }

    #[test]
    fn test_event_labels() {
        assert_eq!(
            OpInteropEvent::ExecutingMessage.as_str(),
            "executing_message"
        );
        assert_eq!(OpInteropEvent::SentMessage.as_str(), "sent_message");
        assert_eq!(OpInteropEvent::RelayedMessage.as_str(), "relayed_message");
        assert_eq!(OpInteropEvent::SendEth.as_str(), "send_eth");
        assert_eq!(OpInteropEvent::RelayEth.as_str(), "relay_eth");
    }
}
