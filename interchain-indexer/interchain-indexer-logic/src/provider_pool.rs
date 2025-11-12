use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use async_trait::async_trait;
use futures::future;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;

use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, direct::NotKeyed},
};

use alloy::{network::Ethereum, providers::Provider};

/// Abstracts the minimal RPC we need for the pool.
#[async_trait]
pub trait RpcProvider: Send + Sync + 'static {
    async fn get_block_number(&self) -> Result<u64>;

    async fn request(&self, method: &str, params: Value) -> Result<Value>;
}

/// Wrapper around real alloy Provider to implement RpcProvider trait.
#[derive(Clone)]
pub struct RealRpcProvider {
    inner: Arc<dyn Provider<Ethereum> + Send + Sync>,
}

impl RealRpcProvider {
    pub fn new(provider: Box<dyn Provider<Ethereum> + Send + Sync>) -> Self {
        Self {
            inner: Arc::from(provider),
        }
    }
}

#[async_trait]
impl RpcProvider for RealRpcProvider {
    async fn get_block_number(&self) -> Result<u64> {
        self.inner
            .get_block_number()
            .await
            .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))
    }

    async fn request(&self, method: &str, _params: Value) -> Result<Value> {
        // For now, we'll use the provider's built-in methods for common operations
        // For generic JSON-RPC requests, we'd need to implement a full JSON-RPC client
        // This is a simplified version - in production you might want to use alloy's RPC methods directly
        match method {
            "eth_blockNumber" => {
                let block_number = self.get_block_number().await?;
                // Convert to JSON-RPC response format
                Ok(serde_json::json!(format!("0x{:x}", block_number)))
            }
            _ => Err(anyhow::anyhow!("Unsupported method: {}", method)),
        }
    }
}

/// Per-node static config.
#[derive(Clone)]
pub struct NodeConfig {
    pub name: String,
    pub http_url: String,
    pub max_rps: u32,         // max requests per second for this node
    pub error_threshold: u32, // consecutive errors before temporary cooldown
    pub cooldown_threshold: u32, // the number of cooldowns before exiting the role of the primary node
    pub cooldown: Duration,   // how long node stays disabled
}

/// Global pool config.
#[derive(Clone)]
pub struct PoolConfig {
    /// How often health task runs (block height refresh, revive attempts, etc.)
    pub health_period: Duration,
    /// Maximum allowed lag vs. observed network head (in blocks). If a node lags more, it's skipped.
    pub max_block_lag: u64,
}

struct Node {
    cfg: NodeConfig,
    provider: Arc<dyn RpcProvider>,
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>, // per-node rate limiter
    state: RwLock<NodeState>,                                    // health & last block snapshot
}

#[derive(Clone, Copy, Debug)]
struct NodeState {
    disabled_until: Option<Instant>,
    consecutive_errors: u32,
    cooldowns_count: u32,
    last_block: Option<u64>,
    last_block_ts: Option<Instant>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            disabled_until: None,
            consecutive_errors: 0,
            cooldowns_count: 0,
            last_block: None,
            last_block_ts: None,
        }
    }
}

#[derive(Error, Debug)]
pub enum PoolError {
    #[error("no healthy nodes available")]
    NoHealthyNodes,
    #[error(transparent)]
    Inner(#[from] anyhow::Error),
}

pub struct ProviderPool {
    nodes: Vec<Arc<Node>>,
    cfg: PoolConfig,
    /// The first node is primary by default. It can be switch in case of a lot of errors.
    primary_idx: RwLock<usize>,
    /// Cached network head (max of nodes' last_block). Updated only by the health task.
    cached_head: AtomicU64,
}

impl ProviderPool {
    /// Create pool and spawn internal health task (self-contained).
    /// Uses real HTTP providers from the node config URLs.
    pub async fn new_with_config(node_cfgs: Vec<NodeConfig>, cfg: PoolConfig) -> Result<Arc<Self>> {
        let providers: Vec<Arc<dyn RpcProvider>> = node_cfgs
            .iter()
            .map(|nc| {
                // Build HTTP provider
                let http =
                    alloy::providers::ProviderBuilder::new().connect_http(nc.http_url.parse()?);
                Ok(Arc::new(RealRpcProvider::new(Box::new(http))) as Arc<dyn RpcProvider>)
            })
            .collect::<Result<Vec<_>>>()?;
        Self::new_with_providers(node_cfgs, providers, cfg).await
    }

    /// Helper method to create pool with pre-created RPC providers.
    /// Exposed publicly to initializing with mock providers for testing.
    pub(crate) async fn new_with_providers(
        node_cfgs: Vec<NodeConfig>,
        providers: Vec<Arc<dyn RpcProvider>>,
        cfg: PoolConfig,
    ) -> Result<Arc<Self>> {
        if node_cfgs.len() != providers.len() {
            return Err(anyhow::anyhow!(
                "Mismatch: {} node configs but {} providers",
                node_cfgs.len(),
                providers.len()
            ));
        }

        let nodes: Vec<Arc<Node>> = node_cfgs
            .into_iter()
            .zip(providers.into_iter())
            .map(|(nc, provider)| {
                // Per-node RPS limiter
                let limiter = RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(nc.max_rps.max(1)).unwrap(), // avoid zero
                ));
                Arc::new(Node {
                    cfg: nc,
                    provider,
                    limiter,
                    state: RwLock::new(NodeState::default()),
                })
            })
            .collect();

        let pool = Arc::new(Self {
            nodes,
            cfg,
            primary_idx: RwLock::new(0),
            cached_head: AtomicU64::new(0),
        });

        // Spawn self-contained health task.
        ProviderPool::spawn_health_task(pool.clone());

        Ok(pool)
    }

    /// Spawn an internal Tokio task that periodically updates node health and cached head.
    fn spawn_health_task(pool: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                pool.health_tick().await;
                sleep(pool.cfg.health_period).await;
            }
        });
    }

    /// Health tick: refresh block numbers, revive nodes after cooldown, update cached head.
    async fn health_tick(&self) {
        let initial_head = self.cached_head.load(Ordering::Relaxed);
        let now = Instant::now();

        // Determine which nodes should be probed
        let should_probe_flags: Vec<bool> = self
            .nodes
            .iter()
            .map(|n| {
                let mut st = n.state.write();

                // If cooldown expired, allow trying the node again.
                if let Some(until) = st.disabled_until {
                    if now >= until {
                        st.disabled_until = None;
                        st.consecutive_errors = 0;
                    }
                }

                // Check if we should probe (node is not disabled)
                st.disabled_until.is_none()
            })
            .collect();

        // Process async operations simultaneously for all nodes that should be probed
        // Collect futures along with their node indices for proper mapping
        let (indices, block_number_futures): (Vec<usize>, Vec<_>) = self
            .nodes
            .iter()
            .zip(should_probe_flags.iter())
            .enumerate()
            .filter_map(|(idx, (n, &should_probe))| {
                if should_probe {
                    Some((idx, n.provider.get_block_number()))
                } else {
                    None
                }
            })
            .unzip();

        // Await all block number requests in parallel
        let block_number_results = future::join_all(block_number_futures).await;

        // Create a mapping from node index to result for efficient lookup
        let result_map: HashMap<usize, &Result<u64>> = indices
            .iter()
            .zip(block_number_results.iter())
            .map(|(&idx, result)| (idx, result))
            .collect();

        // Process results and update node states
        let max_head = self
            .nodes
            .iter()
            .zip(should_probe_flags.iter())
            .enumerate()
            .fold(initial_head, |current_max, (idx, (n, &should_probe))| {
                if should_probe {
                    if let Some(Ok(bn)) = result_map.get(&idx) {
                        let bn = *bn; // Copy the u64 value
                        let mut st = n.state.write();
                        st.last_block = Some(bn);
                        st.last_block_ts = Some(now);
                        return current_max.max(bn);
                    }
                }
                current_max
            });

        // Update cached network head
        self.cached_head.store(max_head, Ordering::Relaxed);
    }

    /// Determine whether a node is currently available: not in cooldown and within block lag.
    fn is_node_available(&self, node: &Node) -> bool {
        let st = *node.state.read();

        if let Some(until) = st.disabled_until {
            if Instant::now() < until {
                return false;
            }
        }

        // Enforce max block lag if we have both numbers.
        if let (Some(node_bn), Some(head)) = (st.last_block, self.read_cached_head()) {
            if head > node_bn && head - node_bn > self.cfg.max_block_lag {
                return false;
            }
        }

        true
    }

    /// Read cached head; None if zero (not yet measured).
    fn read_cached_head(&self) -> Option<u64> {
        let v = self.cached_head.load(Ordering::Relaxed);
        if v == 0 { None } else { Some(v) }
    }

    /// Mark node error and apply cooldown if threshold exceeded.
    fn mark_error(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors += 1;
        if st.consecutive_errors >= node.cfg.error_threshold {
            st.disabled_until = Some(Instant::now() + node.cfg.cooldown);
            st.consecutive_errors = 0; // reset after entering cooldown
            st.cooldowns_count += 1;
            if st.cooldowns_count >= node.cfg.cooldown_threshold {
                let mut primary = self.primary_idx.write();
                *primary = (*primary + 1) % self.nodes.len();
                tracing::warn!("Primary Node {} has been cooldowned too many times. Switching primary to the next node.", node.cfg.name);
            }
        }
    }

    /// Mark node success (clear errors/cooldown).
    fn mark_ok(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors = 0;
        st.disabled_until = None;
    }

    /// Pick the first available node (over healthy, non-rate-limited nodes).
    fn pick_node(&self) -> Option<Arc<Node>> {
        let len = self.nodes.len();
        if len == 0 {
            return None;
        }

        let start = *self.primary_idx.read();

        // Single pass over all nodes - find first available and non-rate-limited node
        (0..len)
            .map(|i| &self.nodes[(start + i) % len])
            .find(|node| {
                // Health & lag checks
                self.is_node_available(node) && node.limiter.check().is_ok()
            })
            .map(|node| node.clone())
    }

    /// Generic request: execute a JSON-RPC method with a selected provider.
    /// Returns the raw JSON-RPC response as a Value.
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, PoolError> {
        // Try up to N nodes (N = number of nodes)
        for _ in 0..self.nodes.len() {
            let Some(node) = self.pick_node() else {
                // No healthy/non-limited nodes available now.
                // Small backoff to avoid spinning; then retry another pass.
                sleep(Duration::from_millis(10)).await;
                continue;
            };

            // Send request to the selected node
            match node.provider.request(method, params.clone()).await {
                Ok(v) => {
                    self.mark_ok(&node);
                    return Ok(v);
                }
                Err(_) => {
                    // Count error and try the next node.
                    self.mark_error(&node);
                    // Short jitter between attempts.
                    sleep(Duration::from_millis(5)).await;
                }
            }
        }

        Err(PoolError::NoHealthyNodes)
    }

    /// Generic request with automatic deserialization.
    /// Execute a JSON-RPC method and deserialize the response to the specified type.
    pub async fn request_typed<T: DeserializeOwned + Send>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<T, PoolError> {
        let value = self.request(method, params).await?;
        serde_json::from_value(value).map_err(|e| {
            PoolError::Inner(anyhow::anyhow!("Failed to deserialize response: {:?}", e))
        })
    }

    /// Get the current block number from a healthy node.
    pub async fn get_block_number(&self) -> Result<u64, PoolError> {
        // Try up to N nodes (N = number of nodes)
        for _ in 0..self.nodes.len() {
            let Some(node) = self.pick_node() else {
                // No healthy/non-limited nodes available now.
                // Small backoff to avoid spinning; then retry another pass.
                sleep(Duration::from_millis(10)).await;
                continue;
            };

            match node.provider.get_block_number().await {
                Ok(v) => {
                    self.mark_ok(&node);
                    return Ok(v);
                }
                Err(_) => {
                    // Count error and try the next node.
                    self.mark_error(&node);
                    // Short jitter between attempts.
                    sleep(Duration::from_millis(5)).await;
                }
            }
        }

        Err(PoolError::NoHealthyNodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{
        MockRpcProvider, create_pool_with_mock_providers, create_test_node,
        create_test_node_with_id, create_test_pool_config,
    };

    #[tokio::test]
    async fn test_pool_creation() {
        let node_cfgs = vec![create_test_node_with_id(1), create_test_node_with_id(2)];
        let pool_cfg = create_test_pool_config();

        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_pool_creation_empty_nodes() {
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(vec![], pool_cfg).await;
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_pool_creation_invalid_url() {
        let mut cfg = create_test_node();
        cfg.http_url = "not-a-url".to_string();
        let pool_cfg = create_test_pool_config();

        let pool = ProviderPool::new_with_config(vec![cfg], pool_cfg).await;
        assert!(pool.is_err());
    }

    #[tokio::test]
    async fn test_request_with_no_nodes() {
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(vec![], pool_cfg)
            .await
            .unwrap();

        let result = pool.request("eth_blockNumber", serde_json::json!([])).await;

        assert!(matches!(result, Err(PoolError::NoHealthyNodes)));
    }

    #[tokio::test]
    async fn test_get_block_number() {
        let mock = MockRpcProvider::new(12345);
        let node_cfgs = vec![create_test_node()];
        let pool_cfg = create_test_pool_config();
        let pool = create_pool_with_mock_providers(node_cfgs, vec![mock], pool_cfg)
            .await
            .unwrap();

        let result = pool.get_block_number().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12345);
    }

    #[tokio::test]
    async fn test_health_tick_updates_cached_head() {
        let node_cfgs = vec![create_test_node()];
        let mock = MockRpcProvider::new(42);
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };
        let pool = create_pool_with_mock_providers(node_cfgs, vec![mock], pool_cfg)
            .await
            .unwrap();

        // Wait a bit for health tick to run
        sleep(Duration::from_millis(150)).await;

        // Cached head might still be 0 if provider fails, but the method should work
        let head = pool.read_cached_head();
        assert_eq!(head, Some(42));
    }

    #[tokio::test]
    async fn test_multiple_nodes() {
        let mocks = vec![
            MockRpcProvider::new(1000),
            MockRpcProvider::new(2000),
            MockRpcProvider::new(3000),
        ];
        let node_cfgs = vec![
            create_test_node_with_id(1),
            create_test_node_with_id(2),
            create_test_node_with_id(3),
        ];
        let pool_cfg = create_test_pool_config();
        let pool = create_pool_with_mock_providers(node_cfgs, mocks, pool_cfg)
            .await
            .unwrap();

        // Make multiple requests in parallel to check the rate limiter
        let requests: Vec<_> = (0..30)
            .map(|_| pool.get_block_number())
            .collect();
        let results = future::join_all(requests).await;

        // Verify all requests succeeded
        let block_numbers: Vec<u64> = results
            .into_iter()
            .map(|r| r.expect("Request should succeed"))
            .collect();

        // Verify we got different results (should distribute across nodes)
        let unique_values: std::collections::HashSet<u64> = block_numbers.iter().copied().collect();
        assert!(
            unique_values.len() == 3,
            "RateLimiter should distribute requests across nodes. Got: {:?}",
            block_numbers
        );
    }

    #[tokio::test]
    async fn test_zero_max_rps_handled() {
        let mut node_cfg = create_test_node();
        node_cfg.max_rps = 0; // Should be handled (maxed to 1)

        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(vec![node_cfg], pool_cfg).await;

        // Should still create successfully (max_rps is maxed to 1)
        assert!(pool.is_ok());
    }

    #[tokio::test]
    async fn test_mock_provider_deterministic_health_check() {
        // Create mock providers with deterministic block numbers
        let mock1 = MockRpcProvider::new(1000);
        let mock2 = MockRpcProvider::new(1005);

        let node_cfgs = vec![create_test_node_with_id(1), create_test_node_with_id(2)];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };

        let pool = create_pool_with_mock_providers(node_cfgs, vec![mock1, mock2], pool_cfg)
            .await
            .unwrap();

        // Wait for health tick to run
        sleep(Duration::from_millis(150)).await;

        // Check that cached head is set to the max block number (1005)
        let head = pool.read_cached_head();
        assert!(head.is_some());
        assert_eq!(head.unwrap(), 1005);
    }

    #[tokio::test]
    async fn test_mock_provider_update_block_number() {
        let mock = MockRpcProvider::new(1000);
        let mock_clone = mock.clone();

        let node_cfgs = vec![create_test_node()];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };

        let pool = create_pool_with_mock_providers(node_cfgs, vec![mock], pool_cfg)
            .await
            .unwrap();

        // Wait for initial health check
        sleep(Duration::from_millis(150)).await;
        assert_eq!(pool.read_cached_head(), Some(1000));

        // Update mock block number
        mock_clone.set_block_number(2000);

        // Wait for next health check
        sleep(Duration::from_millis(150)).await;
        assert_eq!(pool.read_cached_head(), Some(2000));
    }

    #[tokio::test]
    async fn test_mock_provider_failure() {
        let mock = MockRpcProvider::new(1000);
        mock.set_should_fail(true);

        let node_cfgs = vec![create_test_node()];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };

        let pool = create_pool_with_mock_providers(node_cfgs, vec![mock.clone()], pool_cfg)
            .await
            .unwrap();

        // Wait for health check
        sleep(Duration::from_millis(150)).await;

        // Head should remain None since the provider fails
        let head = pool.read_cached_head();
        assert!(head.is_none());

        // Restore the mock provider (set should_fail to false)
        mock.set_should_fail(false);

        // Wait for next health check
        sleep(Duration::from_millis(150)).await;

        // Now the block number should be retrieved correctly
        let head_after_restore = pool.read_cached_head();
        assert_eq!(head_after_restore, Some(1000));

        // Also verify we can get block number directly
        let block_number = pool.get_block_number().await;
        assert_eq!(block_number.unwrap(), 1000);
    }
}
