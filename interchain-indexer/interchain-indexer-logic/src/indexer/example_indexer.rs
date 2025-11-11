use anyhow::Error;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::{error, info, warn};

use crate::{InterchainDatabase, ProviderPool};
use crate::indexer::crosschain_indexer::CrosschainIndexer;

/// Example implementation of CrosschainIndexer trait.
#[allow(dead_code)]
pub struct ExampleIndexer {
    db: Arc<InterchainDatabase>,
    bridge_id: i32,
    providers: HashMap<u64, Arc<ProviderPool>>,
    /// Flag to control the indexing loop
    is_running: Arc<AtomicBool>,
    /// Handle to the indexing task for graceful shutdown
    indexing_handle: parking_lot::RwLock<Option<JoinHandle<()>>>,
}

impl CrosschainIndexer for ExampleIndexer {
    fn new(
        db: Arc<InterchainDatabase>,
        bridge_id: i32,
        providers: HashMap<u64, Arc<ProviderPool>>,
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
            is_running: Arc::new(AtomicBool::new(false)),
            indexing_handle: parking_lot::RwLock::new(None),
        })
    }

    fn start_indexing(&self) -> Result<(), Error> {
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

        // Spawn the indexing task
        let handle = tokio::spawn(async move {
            info!(bridge_id = bridge_id, "Indexing task started");

            // Main indexing loop
            while is_running.load(Ordering::Acquire) {
                match Self::indexing_loop_iteration(&db, bridge_id, &providers).await {
                    Ok(_) => {
                        // Successfully processed, wait before next iteration
                        sleep(Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        error!(
                            bridge_id = bridge_id,
                            err = ?e,
                            "Error in indexing loop iteration"
                        );
                        // Wait a bit longer on error before retrying
                        sleep(Duration::from_secs(5)).await;
                    }
                }
            }

            info!(bridge_id = bridge_id, "Indexing task stopped");
        });

        // Store the handle for later cleanup
        *self.indexing_handle.write() = Some(handle);

        Ok(())
    }

    fn stop_indexing(&self) -> Result<(), Error> {
        if !self.is_running.load(Ordering::Acquire) {
            warn!(bridge_id = self.bridge_id, "Indexer is not running");
            return Ok(());
        }

        info!(bridge_id = self.bridge_id, "Stopping ExampleIndexer");

        // Signal the indexing loop to stop
        self.is_running.store(false, Ordering::Release);

        // Wait for the task to finish
        if let Some(handle) = self.indexing_handle.write().take() {
            // Note: In a real implementation, you might want to use a timeout here
            // For now, we just abort the task if it doesn't finish quickly
            handle.abort();
        }

        Ok(())
    }
}

impl ExampleIndexer {
    /// Single iteration of the indexing loop.
    /// This is where the actual indexing logic would go.
    #[allow(dead_code)]
    async fn indexing_loop_iteration(
        db: &Arc<InterchainDatabase>,
        bridge_id: i32,
        providers: &HashMap<u64, Arc<ProviderPool>>,
    ) -> Result<(), Error> {
        // Example: Get bridge contracts for this bridge
        let contracts = db.get_bridge_contracts(bridge_id).await?;

        info!(
            bridge_id = bridge_id,
            contract_count = contracts.len(),
            "Processing bridge contracts"
        );

        // Example: Process each contract on its respective chain
        for contract in contracts {
            // Convert i64 chain_id to u64 for HashMap lookup
            let chain_id_u64 = contract.chain_id as u64;
            if let Some(_provider_pool) = providers.get(&chain_id_u64) {
                // Example: Use the provider pool to make requests
                // In a real implementation, you would:
                // 1. Get the current block number
                // 2. Query for events/logs from the bridge contract
                // 3. Process and store cross-chain messages/transfers
                //
                // Example usage pattern:
                // let block_number = provider_pool
                //     .request(|provider| async move {
                //         provider.get_block_number().await
                //     })
                //     .await?;
                //
                // Then use the block number to query for events and process them

                // Placeholder: In a real implementation, you would process the results here
                // and store cross-chain messages/transfers in the database
                info!(
                    bridge_id = bridge_id,
                    chain_id = contract.chain_id,
                    contract_address = ?contract.address,
                    "Would process contract here"
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
