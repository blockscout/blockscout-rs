// src/test_utils.rs

use std::{sync::Arc, time::Duration};

use anyhow::Result;
use alloy::{
    providers::{DynProvider, Provider, ProviderBuilder},
    network::Ethereum,
    transports::mock::Asserter,
};
use crate::{NodeConfig, PoolConfig, ProviderPool};

/// Create a basic test node config with deterministic params.
pub fn create_test_node() -> NodeConfig {
    NodeConfig {
        name: "test-node".to_string(),
        http_url: "http://localhost".to_string(), // not used in mock mode
        max_rps: 10,
        error_threshold: 3,
        cooldown_threshold: 3,
        cooldown: Duration::from_millis(100),
    }
}

pub fn create_test_node_with_id(id: u32) -> NodeConfig {
    let mut cfg = create_test_node();
    cfg.name = format!("test-node-{id}");
    cfg
}

pub fn create_test_pool_config() -> PoolConfig {
    PoolConfig {
        health_period: Duration::from_millis(50),
        max_block_lag: 20,
        retry_count: 3,
        retry_initial_delay_ms: 10,
        retry_max_delay_ms: 50,
    }
}

/// Build a DynProvider<Ethereum> using a mock Asserter.
/// The Asserter is returned so the test can push expected requests/responses into it.
pub fn create_mock_provider_with_asserter() -> (Asserter, DynProvider<Ethereum>) {
    let asserter = Asserter::new();

    let provider = ProviderBuilder::new()
        .connect_mocked_client(asserter.clone())
        .erased();

    (asserter, provider)
}

/// Create a pool from NodeConfigs and mock providers built from Asserter.
/// Each node config will be paired with a provider in the same index.
pub async fn create_pool_with_mock_providers(
    node_cfgs: Vec<NodeConfig>,
    providers: Vec<DynProvider<Ethereum>>,
    cfg: PoolConfig,
) -> Result<Arc<ProviderPool>> {
    ProviderPool::new_with_providers(node_cfgs, providers, cfg).await
}
