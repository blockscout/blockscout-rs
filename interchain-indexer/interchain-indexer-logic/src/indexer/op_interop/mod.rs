// SPDX-License-Identifier: LicenseRef-Blockscout

//! OP Stack native interop indexer: `L2ToL2CrossDomainMessenger` messages
//! between the chains of one dependency set, read up to a confirmed block tag
//! and from logs alone. See ADR-018.

mod abi;
mod consolidation;
mod eth_transfer;
mod events;
pub mod indexer;
mod lifecycle;
pub(crate) mod metrics;
mod overlay;
pub mod settings;
mod sweep;
mod types;
mod version;

pub use indexer::{OpInteropChainConfig, OpInteropContractConfig, OpInteropIndexer};
pub use settings::OpInteropIndexerSettings;

/// The simulated devnet fixtures (see `tests/fixtures/op_interop/README.md`)
/// and a registry matching the committed devnet config, shared by the unit
/// tests of this module.
#[cfg(test)]
pub(crate) mod test_fixtures {
    use std::collections::HashSet;

    use alloy::{primitives::ChainId, rpc::types::Log};

    use super::{
        abi::OpInteropRegistry,
        indexer::OpInteropContractConfig,
        version::{
            CROSS_L2_INBOX, EXECUTING_MESSAGE_TOPIC0, L2_TO_L2_MESSENGER, RELAY_ETH_TOPIC0,
            RELAYED_MESSAGE_TOPIC0, SEND_ETH_TOPIC0, SENT_MESSAGE_TOPIC0, SUPERCHAIN_ETH_BRIDGE,
        },
    };

    /// `sepolia-devnet-4`.
    pub(crate) const SOURCE_CHAIN_ID: ChainId = 420_130_019;
    /// `sepolia-devnet-5`.
    pub(crate) const DESTINATION_CHAIN_ID: ChainId = 420_130_020;
    pub(crate) const SOURCE_START_BLOCK: u64 = 465_679;
    pub(crate) const DESTINATION_START_BLOCK: u64 = 829_213;

    const SIMULATED_SEND: &str =
        include_str!("../../../tests/fixtures/op_interop/simulated_send_devnet4.json");
    const SIMULATED_RELAY: &str =
        include_str!("../../../tests/fixtures/op_interop/simulated_relay_devnet5.json");

    /// The logs of every call of a simulated block, one `Vec` per call.
    fn logs_by_call(raw: &str) -> Vec<Vec<Log>> {
        let block: serde_json::Value = serde_json::from_str(raw).expect("fixture is JSON");
        block["calls"]
            .as_array()
            .expect("fixture block has calls")
            .iter()
            .map(|call| {
                serde_json::from_value(call["logs"].clone()).expect("fixture logs are RPC logs")
            })
            .collect()
    }

    /// The send fixture: `sendETH` (three logs), then a plain `sendMessage`
    /// (one log). Each call is its own transaction.
    pub(crate) fn send_calls() -> Vec<Vec<Log>> {
        logs_by_call(SIMULATED_SEND)
    }

    /// The relay fixture: one `relayMessage` transaction (four logs).
    pub(crate) fn relay_logs() -> Vec<Log> {
        logs_by_call(SIMULATED_RELAY).remove(0)
    }

    fn log_with_topic0(logs: Vec<Log>, topic0: alloy::primitives::B256) -> Log {
        logs.into_iter()
            .find(|log| log.topic0() == Some(&topic0))
            .expect("fixture holds the event")
    }

    /// The `SentMessage` logs of message 0 and message 1.
    pub(crate) fn sent_message_logs() -> [Log; 2] {
        let mut calls = send_calls();
        let second = log_with_topic0(calls.remove(1), SENT_MESSAGE_TOPIC0);
        let first = log_with_topic0(calls.remove(0), SENT_MESSAGE_TOPIC0);
        [first, second]
    }

    pub(crate) fn executing_message_log() -> Log {
        log_with_topic0(relay_logs(), EXECUTING_MESSAGE_TOPIC0)
    }

    /// The fixture `ExecutingMessage` at `log_index`, with `Identifier.origin`
    /// (data word 0) set to `0x9999…99`: a foreign-origin read.
    pub(crate) fn foreign_read(log_index: u64) -> Log {
        let mut log = executing_message_log();
        log.log_index = Some(log_index);
        let mut data = log.inner.data.data.to_vec();
        data[..32].fill(0);
        data[12..32].fill(0x99);
        log.inner.data = alloy::primitives::LogData::new_unchecked(
            log.inner.data.topics().to_vec(),
            data.into(),
        );
        log
    }

    /// The fixture `ExecutingMessage` at `log_index`, reading another log of the
    /// same source block: `Identifier.logIndex` (data word 2) =
    /// `source_log_index` and `msgHash` (topic 1) = `marker`. Messenger origin
    /// and source chain are unchanged, so it is a candidate of a relay in its
    /// window, but it never matches message 0's source.
    pub(crate) fn pull_read(
        log_index: u64,
        source_log_index: u64,
        marker: alloy::primitives::B256,
    ) -> Log {
        let mut log = executing_message_log();
        log.log_index = Some(log_index);
        let mut topics = log.inner.data.topics().to_vec();
        topics[1] = marker;
        let mut data = log.inner.data.data.to_vec();
        data[64..96]
            .copy_from_slice(&alloy::primitives::U256::from(source_log_index).to_be_bytes::<32>());
        log.inner.data = alloy::primitives::LogData::new_unchecked(topics, data.into());
        log
    }

    pub(crate) fn relayed_message_log() -> Log {
        log_with_topic0(relay_logs(), RELAYED_MESSAGE_TOPIC0)
    }

    /// The `SendETH` log of the send fixture's first call.
    pub(crate) fn send_eth_log() -> Log {
        log_with_topic0(send_calls().remove(0), SEND_ETH_TOPIC0)
    }

    /// The `RelayETH` log of the relay fixture.
    pub(crate) fn relay_eth_log() -> Log {
        log_with_topic0(relay_logs(), RELAY_ETH_TOPIC0)
    }

    /// The three contract entries of one chain, as `bridges.json` carries them.
    pub(crate) fn contracts(started_at_block: u64) -> Vec<OpInteropContractConfig> {
        vec![
            OpInteropContractConfig {
                address: CROSS_L2_INBOX,
                kind: Some("cross_l2_inbox".to_string()),
                version: 1,
                started_at_block,
            },
            OpInteropContractConfig {
                address: L2_TO_L2_MESSENGER,
                kind: Some("l2_to_l2_messenger".to_string()),
                version: 1,
                started_at_block,
            },
            OpInteropContractConfig {
                address: SUPERCHAIN_ETH_BRIDGE,
                kind: Some("superchain_eth_bridge".to_string()),
                version: 1,
                started_at_block,
            },
        ]
    }

    pub(crate) fn registry() -> OpInteropRegistry {
        let source = contracts(SOURCE_START_BLOCK);
        let destination = contracts(DESTINATION_START_BLOCK);
        OpInteropRegistry::from_chain_contracts([
            (SOURCE_CHAIN_ID as i64, source.as_slice()),
            (DESTINATION_CHAIN_ID as i64, destination.as_slice()),
        ])
        .expect("fixture registry builds")
    }

    pub(crate) fn configured_chains() -> HashSet<ChainId> {
        HashSet::from([SOURCE_CHAIN_ID, DESTINATION_CHAIN_ID])
    }
}
