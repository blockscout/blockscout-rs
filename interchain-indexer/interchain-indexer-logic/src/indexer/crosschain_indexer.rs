// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
};

use anyhow::Error;
use chrono::NaiveDateTime;
use serde_json::Value;
use tonic::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrosschainIndexerState {
    Idle,           // indexer is alive, but not indexing at the moment (not started yet or stopped)
    Running,        // indexer is in operation state
    Failed(String), // indexer has stopped due to an unrecoverable error
}

impl Display for CrosschainIndexerState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CrosschainIndexerState::Idle => write!(f, "idle"),
            CrosschainIndexerState::Running => write!(f, "running"),
            CrosschainIndexerState::Failed(e) => write!(f, "failed: {}", e),
        }
    }
}

pub struct CrosschainIndexerStatus {
    pub state: CrosschainIndexerState, // current state of the indexer
    /// Timestamp when indexer instance was initialized.
    pub init_timestamp: NaiveDateTime,

    /// Arbitrary indexer-specific data.
    ///
    /// Prefer keeping this small and stable (control-plane info useful for debugging),
    /// and avoid putting high-cardinality or frequently-changing data here.
    pub extra_info: HashMap<String, Value>,
    // TODO: Telemetry fields were intentionally removed from the trait status.
    // They should be exported as Prometheus metrics instead. Status as of the
    // indexing-completeness-and-progress-api work:
    // - catchup progress / lag:
    //   - `indexer_catchup_progress{bridge_id,chain_id}` (gauge) — DONE, exported
    //     as `interchain_indexer_catchup_progress` (`indexer::metrics`), refreshed
    //     by `spawn_indexing_progress_metrics_worker`
    //     (`interchain-indexer-server/src/server.rs`) from the same DB-only join
    //     the `GetIndexingProgress` RPC uses. `interchain_indexer_catchup_blocks_remaining`
    //     ships alongside it, and the failed-range ledger separately exports
    //     `interchain_indexer_failed_blocks` / `interchain_indexer_oldest_open_hole_age_seconds`
    //     per `(bridge_id, chain_id)` from the same worker.
    //   - `indexer_chain_lag_blocks{bridge_id,chain_id}` — still unimplemented; this
    //     was an alternative to catchup progress, not an additional requirement.
    // - counts: still unimplemented
    //   - `indexer_messages_indexed_total{bridge_id,chain_id?}` (counter)
    //   - `indexer_transfers_indexed_total{bridge_id,chain_id?}` (counter)
    //   - `indexer_errors_total{bridge_id,kind?}` (counter) — the failed-range ledger
    //     gauges above cover per-range hole tracking, not a generic error counter
}

#[async_trait]
pub trait CrosschainIndexer: Send + Sync {
    // the indexer for the bridge will be selected based on this name (bridges.json -> indexer field)
    fn name(&self) -> String;
    // optional description of the indexer
    fn description(&self) -> String {
        "".to_string()
    }

    // The implementation should perform the necessary initialization,
    // spawn the required tasks, and return from this method as soon as possible.
    // Do not await the completion of any indexing loops or similar operations in this method.
    // Return values:
    //   Ok(()) - the indexer has switched to the Running state
    //   Err(e) - the indexer has failed to start
    async fn start(&self) -> Result<(), Error>;

    // The implementation should gracefully stop the indexing loop with any necessary cleanup.
    // If a graceful stop is not possible, the implementation should forcibly abort it anyway.
    async fn stop(&self);

    // Get the current state of the indexer
    fn get_state(&self) -> CrosschainIndexerState;
    // Get the current status of the indexer
    fn get_status(&self) -> CrosschainIndexerStatus;
}
