use std::collections::HashMap;

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

pub struct CrosschainIndexerStatus {
    pub state: CrosschainIndexerState, // current state of the indexer
    pub catchup_progress: f64,         // 0..1, 1.0 means that catchup is fully completed
    pub init_timestamp: NaiveDateTime, // timestamp when indexer was initialized
    pub messages_indexed: u64,         // from init_timestamp
    pub transfers_indexed: u64,        // from init_timestamp
    pub error_count: u64,              // any errors encountered during indexing
    pub extra_info: HashMap<String, Value>, // arbitrary indexer-specific data
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
    async fn start_indexing(&self) -> Result<(), Error>;

    // The implementation should gracefully stop the indexing loop with any necessary cleanup.
    // If a graceful stop is not possible, the implementation should forcibly abort it anyway.
    async fn stop_indexing(&self);

    // Get the current state of the indexer
    fn get_state(&self) -> CrosschainIndexerState;
    // Get the current status of the indexer
    fn get_status(&self) -> CrosschainIndexerStatus;
}
