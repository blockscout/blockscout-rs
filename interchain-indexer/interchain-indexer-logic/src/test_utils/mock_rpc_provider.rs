use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::Value;

use crate::provider_pool::{NodeConfig, PoolConfig, ProviderPool, RpcProvider};

/// Mock provider for testing.
/// Allows deterministic control over provider behavior.
#[derive(Clone)]
pub struct MockRpcProvider {
    block_number: Arc<RwLock<u64>>,
    should_fail: Arc<RwLock<bool>>,
    responses: Arc<RwLock<HashMap<String, Value>>>,
}

impl MockRpcProvider {
    /// Create a new mock provider with an initial block number.
    pub fn new(initial_block: u64) -> Self {
        Self {
            block_number: Arc::new(RwLock::new(initial_block)),
            should_fail: Arc::new(RwLock::new(false)),
            responses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set the block number that will be returned by get_block_number().
    pub fn set_block_number(&self, block: u64) {
        *self.block_number.write() = block;
    }

    /// Get the current block number.
    pub fn get_block_number_sync(&self) -> u64 {
        *self.block_number.read()
    }

    /// Set whether the provider should fail on requests.
    pub fn set_should_fail(&self, fail: bool) {
        *self.should_fail.write() = fail;
    }

    /// Check if the provider is configured to fail.
    pub fn should_fail(&self) -> bool {
        *self.should_fail.read()
    }

    /// Set a response for a specific method.
    pub fn set_response(&self, method: &str, response: Value) {
        self.responses.write().insert(method.to_string(), response);
    }
}

#[async_trait]
impl RpcProvider for MockRpcProvider {
    async fn get_block_number(&self) -> Result<u64> {
        if self.should_fail() {
            Err(anyhow::anyhow!("Mock provider configured to fail"))
        } else {
            Ok(self.get_block_number_sync())
        }
    }

    async fn request(&self, method: &str, _params: Value) -> Result<Value> {
        if self.should_fail() {
            return Err(anyhow::anyhow!("Mock provider configured to fail"));
        }

        // Check if there's a custom response for this method
        if let Some(response) = self.responses.read().get(method) {
            return Ok(response.clone());
        }

        // Default behavior: for eth_blockNumber, return the current block number
        if method == "eth_blockNumber" {
            let block_number = self.get_block_number_sync();
            return Ok(serde_json::json!(format!("0x{:x}", block_number)));
        }

        Err(anyhow::anyhow!("Unsupported method: {}", method))
    }
}

pub fn create_test_node() -> NodeConfig {
    create_test_node_with_id(1)
}

pub fn create_test_node_with_id(id: u32) -> NodeConfig {
    NodeConfig {
        name: format!("mock-node-{}", id),
        http_url: format!("http://node{}.mock.io/rpc", id),
        max_rps: 10,
        error_threshold: 3,
        cooldown_threshold: 1,
        cooldown: Duration::from_secs(1),
    }
}

pub fn create_test_pool_config() -> PoolConfig {
    PoolConfig {
        health_period: Duration::from_millis(100),
        ..Default::default()
    }
}

/// Create a ProviderPool with mock RPC providers (for testing).
pub async fn create_pool_with_mock_providers(
    node_cfgs: Vec<NodeConfig>,
    mock_providers: Vec<MockRpcProvider>,
    cfg: PoolConfig,
) -> Result<Arc<ProviderPool>> {
    if node_cfgs.len() != mock_providers.len() {
        return Err(anyhow::anyhow!(
            "Mismatch: {} node configs but {} providers",
            node_cfgs.len(),
            mock_providers.len()
        ));
    }

    let providers: Vec<Arc<dyn RpcProvider>> = mock_providers
        .into_iter()
        .map(|p| Arc::new(p) as Arc<dyn RpcProvider>)
        .collect();
    ProviderPool::new_with_providers(node_cfgs, providers, cfg).await
}
