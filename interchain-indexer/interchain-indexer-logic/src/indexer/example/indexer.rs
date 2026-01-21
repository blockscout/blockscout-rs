use alloy::{
    network::Ethereum,
    primitives::{Address, B256, keccak256},
    providers::{DynProvider, Provider},
    rpc::types::eth::Filter,
};
use anyhow::Error;
use chrono::{NaiveDateTime, Utc};
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{task::JoinHandle, time::sleep};
use tonic::async_trait;
use tracing::{info, warn};

use crate::{
    CrosschainIndexerState, CrosschainIndexerStatus, InterchainDatabase,
    example::settings::ExampleIndexerSettings, indexer::crosschain_indexer::CrosschainIndexer,
};

/// Example implementation of CrosschainIndexer trait.
#[allow(dead_code)]
pub struct ExampleIndexer {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    providers: HashMap<u64, DynProvider<Ethereum>>,
    // Indexer-specific settings
    settings: ExampleIndexerSettings,
    // Flag to control the indexing loop
    is_running: Arc<AtomicBool>,
    /// Handle to the indexing task for graceful shutdown
    indexing_handle: parking_lot::RwLock<Option<JoinHandle<()>>>,

    status: parking_lot::RwLock<CrosschainIndexerState>,
    init_timestamp: NaiveDateTime,
    fake_counter: Arc<AtomicU64>,
    fake_err_counter: Arc<AtomicU64>,
}

impl ExampleIndexer {
    pub fn new(
        db: Arc<InterchainDatabase>,
        bridge_id: i32,
        providers: HashMap<u64, DynProvider<Ethereum>>,
        settings: ExampleIndexerSettings,
    ) -> Result<Self, Error> {
        info!(
            bridge_id = bridge_id,
            chain_count = providers.len(),
            "Creating ExampleIndexer"
        );

        Ok(Self {
            db,
            bridge_id,
            providers,
            settings,
            is_running: Arc::new(AtomicBool::new(false)),
            indexing_handle: parking_lot::RwLock::new(None),
            status: parking_lot::RwLock::new(CrosschainIndexerState::Idle),
            init_timestamp: Utc::now().naive_utc(),
            fake_counter: Arc::new(AtomicU64::new(0)),
            fake_err_counter: Arc::new(AtomicU64::new(0)),
        })
    }
}

fn single_block_transfer_filter(block_number: u64) -> Filter {
    let topic = B256::from(keccak256("Transfer(address,address,uint256)".as_bytes()));

    Filter::new()
        .from_block(block_number)
        .to_block(block_number)
        .event_signature(topic)
}

#[async_trait]
impl CrosschainIndexer for ExampleIndexer {
    fn name(&self) -> String {
        "ExampleIndexer".to_string()
    }

    fn description(&self) -> String {
        "Example indexer to demonstrate the CrosschainIndexer trait".to_string()
    }

    async fn start(&self) -> Result<(), Error> {
        if self.is_running.load(Ordering::Acquire) {
            warn!(bridge_id = self.bridge_id, "Indexer is already running");
            return Ok(());
        }

        info!(bridge_id = self.bridge_id, "Starting ExampleIndexer");

        self.is_running.store(true, Ordering::Release);

        let db = self.db.clone();
        let bridge_id = self.bridge_id;
        let providers = self.providers.clone();
        let is_running = self.is_running.clone();
        let fake_counter = self.fake_counter.clone();
        let fake_err_counter = self.fake_err_counter.clone();

        let fetch_interval = self.settings.fetch_interval;

        // Spawn the indexing task
        let handle = tokio::spawn(async move {
            info!(bridge_id = bridge_id, "Indexing task started");

            // Main indexing loop
            while is_running.load(Ordering::Acquire) {
                match Self::indexing_loop_iteration(&db, bridge_id, &providers).await {
                    Ok(_) => {
                        // Successfully processed
                        fake_counter.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        tracing::error!(
                            bridge_id = bridge_id,
                            err = ?e,
                            "Error in indexing loop iteration"
                        );

                        fake_err_counter.fetch_add(1, Ordering::Relaxed);
                    }
                }

                // Wait before next iteration
                sleep(fetch_interval).await;
            }

            info!(bridge_id = bridge_id, "Indexing task stopped");
        });

        // Store the handle for later cleanup
        *self.indexing_handle.write() = Some(handle);
        *self.status.write() = CrosschainIndexerState::Running;

        Ok(())
    }

    async fn stop(&self) {
        if !self.is_running.load(Ordering::Acquire) {
            warn!(bridge_id = self.bridge_id, "Indexer is not running");
            return;
        }

        info!(bridge_id = self.bridge_id, "Stopping ExampleIndexer");

        // Signal the indexing loop to stop
        self.is_running.store(false, Ordering::Release);

        // add delay to ensure the indexing loop is stopped
        sleep(Duration::from_secs(1)).await;

        // Wait for the task to finish
        if let Some(handle) = self.indexing_handle.write().take() {
            // Note: In a real implementation, you might want to use a timeout here
            // For now, we just abort the task if it doesn't finish quickly
            handle.abort();
        }

        info!(bridge_id = self.bridge_id, "ExampleIndexer stopped");

        *self.status.write() = CrosschainIndexerState::Idle;
    }

    fn get_state(&self) -> CrosschainIndexerState {
        self.status.read().clone()
    }

    fn get_status(&self) -> CrosschainIndexerStatus {
        CrosschainIndexerStatus {
            state: self.status.read().clone(),
            init_timestamp: self.init_timestamp,
            extra_info: HashMap::from([
                ("foo".to_string(), serde_json::json!("bar")),
                (
                    "chains_count".to_string(),
                    serde_json::json!(self.providers.len()),
                ),
                ("the_main_answer".to_string(), serde_json::json!(42)),
            ]),
        }
    }
}

impl ExampleIndexer {
    /// Single iteration of the indexing loop.
    /// This is where the actual indexing logic would go.
    #[allow(dead_code)]
    async fn indexing_loop_iteration(
        db: &Arc<InterchainDatabase>,
        bridge_id: i32,
        providers: &HashMap<u64, DynProvider<Ethereum>>,
    ) -> Result<(), Error> {
        // Example: Get bridge contracts for this bridge
        let contracts = db.get_bridge_contracts(bridge_id).await?;

        info!(
            bridge_id = bridge_id,
            contract_count = contracts.len(),
            "Processing bridge contracts"
        );

        let mut prev_block_number = 0;

        // Example: Process each contract on its respective chain
        for contract in contracts {
            // Convert i64 chain_id to u64 for HashMap lookup
            let chain_id_u64 = contract.chain_id as u64;
            if let Some(provider) = providers.get(&chain_id_u64) {
                let provider = provider.clone();

                let block_number = provider.get_block_number().await?;
                let chain_id = provider.get_chain_id().await?;

                let test_address = Address::from_str("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")?;

                // Concurrently fetch block number and balance as an example of batching.
                let (bn_res, balance_res) = tokio::join!(
                    provider.get_block_number(),
                    provider.get_balance(test_address)
                );

                let bn = bn_res?;
                let balance = balance_res?;

                let transfers_cnt = if prev_block_number != 0 {
                    let filter = single_block_transfer_filter(block_number);
                    let logs = provider.get_logs(&filter).await?;
                    logs.len()
                } else {
                    0
                };

                prev_block_number = block_number;

                tracing::info!(
                    bridge_id = bridge_id,
                    chain_id =? chain_id,
                    block_number = block_number,
                    balance =? balance,
                    bn =? bn,
                    transfers_cnt =? transfers_cnt,
                    "Indexer example processing iteration"
                );
            } else {
                warn!(
                    bridge_id = bridge_id,
                    chain_id = contract.chain_id,
                    "No provider pool found for chain"
                );
            }
        }

        Ok(())
    }
}
