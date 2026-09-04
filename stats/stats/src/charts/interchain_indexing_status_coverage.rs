// SPDX-License-Identifier: LicenseRef-Blockscout

//! The interchain catch-up axis' coverage guard.
//!
//! Modelled directly on [`crate::charts::interchain_filter_coverage`] — two
//! layers, for the same reason that module's docs give: a chart declaring the
//! axis is not itself checkable by the compiler (nothing fails to compile if a
//! new interchain chart forgets `indexing_status_requirement()`), so it has to
//! be a registry a test walks.
//!
//! - **Layer 1** proves that every `Properties` type *in the registry* declares
//!   `interchain: InterchainIndexingStatus::CaughtUp`.
//! - **Layer 2** proves that every interchain chart id in
//!   `config/interchain/charts.json` is covered by the registry. Since a chart
//!   that is not in `charts.json` cannot be enabled at all, a *new* interchain
//!   chart cannot reach production without passing through here — this is the
//!   test that stops a new interchain chart from quietly opting out, which is
//!   the failure mode that makes the whole feature inert.
//! - **Layer 3** is the mirror image: three representative non-interchain
//!   charts must NOT carry the axis, catching a copy-paste that would make a
//!   Blockscout-mode chart wait on an interchain indexer that does not exist in
//!   that mode.

#![cfg(test)]

use crate::{
    charts::{
        ChartProperties, Named,
        indexing_status::{IndexingStatusTrait, InterchainIndexingStatus},
        interchain_filter_coverage::snake_to_camel,
    },
    counters::{
        TotalZetachainCrossChainTxns,
        interchain::{
            TotalInterchainMessages, TotalInterchainMessagesReceived, TotalInterchainMessagesSent,
            TotalInterchainTransferUsers, TotalInterchainTransfers,
            TotalInterchainTransfersReceived, TotalInterchainTransfersSent,
        },
        multichain::TotalMultichainTxns,
    },
    lines::{
        NewTxns,
        interchain::{
            messages_growth_received_interchain::MessagesGrowthReceivedInterchain,
            messages_growth_sent_interchain::MessagesGrowthSentInterchain,
            new_messages_interchain::NewMessagesInterchain,
            new_messages_received_interchain::NewMessagesReceivedInterchain,
            new_messages_sent_interchain::NewMessagesSentInterchain,
            new_transfers_interchain::NewTransfersInterchain,
            new_transfers_received_interchain::NewTransfersReceivedInterchain,
            new_transfers_sent_interchain::NewTransfersSentInterchain,
        },
    },
};

struct CoverageEntry {
    chart_name: String,
    declares_caught_up: bool,
}

macro_rules! coverage_entries {
    ($($ty:ty),+ $(,)?) => {
        vec![$(CoverageEntry {
            chart_name: <$ty as Named>::name(),
            declares_caught_up: <$ty as ChartProperties>::indexing_status_requirement().interchain
                == InterchainIndexingStatus::CaughtUp,
        }),+]
    };
}

/// All 15 interchain chart families. The failure message names the one-line fix
/// (add `.with_interchain(InterchainIndexingStatus::CaughtUp)` /
/// `fn indexing_status_requirement()`) rather than leaving the reader to
/// rediscover it.
fn registry() -> Vec<CoverageEntry> {
    coverage_entries![
        TotalInterchainMessages,
        TotalInterchainMessagesSent,
        TotalInterchainMessagesReceived,
        TotalInterchainTransfers,
        TotalInterchainTransfersSent,
        TotalInterchainTransfersReceived,
        TotalInterchainTransferUsers,
        NewMessagesInterchain,
        NewMessagesSentInterchain,
        NewMessagesReceivedInterchain,
        NewTransfersInterchain,
        NewTransfersSentInterchain,
        NewTransfersReceivedInterchain,
        MessagesGrowthSentInterchain,
        MessagesGrowthReceivedInterchain,
    ]
}

#[test]
fn every_interchain_chart_declares_the_interchain_axis() {
    for entry in registry() {
        assert!(
            entry.declares_caught_up,
            "{} does not declare `interchain: InterchainIndexingStatus::CaughtUp` in its \
             `indexing_status_requirement()`. Add \
             `.with_interchain(InterchainIndexingStatus::CaughtUp)` (counters) or a \
             `fn indexing_status_requirement()` override returning it (line charts) — \
             without this, the chart never waits for the interchain catch-up start check and \
             is excluded from `no_non_interchain_chart_declares_the_interchain_axis`'s \
             sibling assertion.",
            entry.chart_name
        );
    }
}

/// The registry covers every interchain chart that can be enabled.
///
/// A new interchain chart must appear in `config/interchain/charts.json` to be
/// enabled at all, so adding one without registering it here fails this test.
/// None of the 15 families remaps `implementation` today (verified against
/// `config/interchain/charts.json`), so the config key equals the served id for
/// all of them; still resolve through `implementation` the same way
/// `interchain_filter_coverage`'s layer 2 does, so this stays correct if that
/// ever changes.
#[test]
fn interchain_axis_registry_covers_every_configured_interchain_chart() {
    let config: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../config/interchain/charts.json"
    )))
    .expect("config/interchain/charts.json is valid JSON");

    let registered: Vec<String> = registry().into_iter().map(|e| e.chart_name).collect();
    let mut configured = Vec::new();
    for section in ["counters", "line_charts"] {
        let charts = config
            .get(section)
            .and_then(|s| s.as_object())
            .unwrap_or_else(|| panic!("charts.json has no `{section}` object"));
        for (key, settings) in charts {
            let served = settings
                .get("implementation")
                .and_then(|i| i.as_str())
                .unwrap_or(key.as_str());
            configured.push((key.clone(), served.to_owned()));
        }
    }
    assert!(
        !configured.is_empty(),
        "charts.json parsed to no charts at all"
    );

    for (config_key, served_key) in configured {
        let chart_name = snake_to_camel(&served_key);
        let via = if served_key == config_key {
            String::new()
        } else {
            format!(" (via `implementation: {served_key}`)")
        };
        assert!(
            registered.contains(&chart_name),
            "interchain chart `{config_key}`{via} serves chart id `{chart_name}`, which is \
             enabled by config/interchain/charts.json but is not in the interchain axis \
             registry in this file. Add its `Properties` type to `registry()`."
        );
    }
}

/// Three representative non-interchain charts must keep the axis at
/// `LEAST_RESTRICTIVE` — catches a copy-paste that would make a chart in
/// another mode wait on an interchain indexer that does not exist there.
#[test]
fn no_non_interchain_chart_declares_the_interchain_axis() {
    assert_eq!(
        NewTxns::indexing_status_requirement().interchain,
        InterchainIndexingStatus::LEAST_RESTRICTIVE,
        "a blockscout-mode line chart must not declare the interchain axis"
    );
    assert_eq!(
        TotalMultichainTxns::indexing_status_requirement().interchain,
        InterchainIndexingStatus::LEAST_RESTRICTIVE,
        "a multichain counter must not declare the interchain axis"
    );
    assert_eq!(
        TotalZetachainCrossChainTxns::indexing_status_requirement().interchain,
        InterchainIndexingStatus::LEAST_RESTRICTIVE,
        "a zetachain-CCTX counter must not declare the interchain axis"
    );
}
