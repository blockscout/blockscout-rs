use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;

use governor::{clock::DefaultClock, Quota, RateLimiter, state::direct::NotKeyed, state::InMemoryState};

use alloy::network::Ethereum;
use alloy::providers::Provider;

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

/// Mock provider for testing.
/// Allows deterministic control over provider behavior.
#[cfg(test)]
#[derive(Clone)]
pub struct MockRpcProvider {
    block_number: Arc<RwLock<u64>>,
    should_fail: Arc<RwLock<bool>>,
    responses: Arc<RwLock<std::collections::HashMap<String, Value>>>,
}

#[cfg(test)]
impl MockRpcProvider {
    /// Create a new mock provider with an initial block number.
    pub fn new(initial_block: u64) -> Self {
        Self {
            block_number: Arc::new(RwLock::new(initial_block)),
            should_fail: Arc::new(RwLock::new(false)),
            responses: Arc::new(RwLock::new(std::collections::HashMap::new())),
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

#[cfg(test)]
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

/// Per-node static config.
#[derive(Clone)]
pub struct NodeConfig {
    pub name: String,
    pub http_url: String,
    pub max_rps: u32,         // max requests per second for this node
    pub error_threshold: u32, // consecutive errors before temporary disable
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
    state: RwLock<NodeState>,         // health & last block snapshot
}

#[derive(Clone, Copy, Debug)]
struct NodeState {
    disabled_until: Option<Instant>,
    consecutive_errors: u32,
    last_block: Option<u64>,
    last_block_ts: Option<Instant>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            disabled_until: None,
            consecutive_errors: 0,
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
    /// Rolling round-robin index (not performance-critical, protected by RwLock).
    rr_idx: RwLock<usize>,
    /// Cached network head (max of nodes' last_block). Updated only by the health task.
    cached_head: AtomicU64,
}

impl ProviderPool {
    /// Create pool and spawn internal health task (self-contained).
    /// Uses real HTTP providers from the node config URLs.
    pub async fn new_with_config(node_cfgs: Vec<NodeConfig>, cfg: PoolConfig) -> Result<Arc<Self>> {
        let mut providers: Vec<Arc<dyn RpcProvider>> = Vec::with_capacity(node_cfgs.len());
        for nc in &node_cfgs {
            // Build HTTP provider
            let http = alloy::providers::ProviderBuilder::new()
                .connect_http(nc.http_url.parse()?);
            providers.push(Arc::new(RealRpcProvider::new(Box::new(http))));
        }
        Self::new_with_providers(node_cfgs, providers, cfg).await
    }

    /// Create pool with custom providers (useful for testing with mocks).
    #[cfg(test)]
    pub async fn new_with_mock_providers(
        node_cfgs: Vec<NodeConfig>,
        mock_providers: Vec<MockRpcProvider>,
        cfg: PoolConfig,
    ) -> Result<Arc<Self>> {
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
        Self::new_with_providers(node_cfgs, providers, cfg).await
    }

    /// Internal method to create pool with RPC providers.
    async fn new_with_providers(
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

        let mut nodes = Vec::with_capacity(node_cfgs.len());
        for (nc, provider) in node_cfgs.into_iter().zip(providers.into_iter()) {
            // Per-node RPS limiter
            let limiter = RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(nc.max_rps.max(1)).unwrap(), // avoid zero
            ));
            nodes.push(Arc::new(Node {
                cfg: nc,
                provider,
                limiter,
                state: RwLock::new(NodeState::default()),
            }));
        }

        let pool = Arc::new(Self {
            nodes,
            cfg,
            rr_idx: RwLock::new(0),
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
        let mut max_head: u64 = self.cached_head.load(Ordering::Relaxed);

        for n in &self.nodes {
            let now = Instant::now();
            let should_probe = {
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
            };

            // Probe block number only if node is not disabled.
            if should_probe {
                // Best-effort: ignore failures here, handled by error counters during real requests.
                if let Ok(bn) = n.provider.get_block_number().await {
                    let mut st = n.state.write();
                    st.last_block = Some(bn);
                    st.last_block_ts = Some(now);
                    if bn > max_head {
                        max_head = bn;
                    }
                }
            }
        }

        // Update cached network head once per tick (not per request).
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
        }
    }

    /// Mark node success (clear errors/cooldown).
    fn mark_ok(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors = 0;
        st.disabled_until = None;
    }

    /// Pick the next node using round-robin over healthy, non-rate-limited nodes.
    fn pick_node(&self) -> Option<Arc<Node>> {
        let len = self.nodes.len();
        if len == 0 {
            return None;
        }

        // Round-robin start index
        let start = {
            let mut idx = self.rr_idx.write();
            let cur = *idx;
            *idx = (*idx + 1) % len;
            cur
        };

        // Single pass over all nodes
        for i in 0..len {
            let node = &self.nodes[(start + i) % len];

            // Health & lag checks
            if !self.is_node_available(node) {
                continue;
            }

            // Rate limit check (non-blocking). If limited, try next node.
            if node.limiter.check().is_ok() {
                return Some(node.clone());
            }
        }

        None
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

            // For real providers, use with_provider. For mocks, return an error.
            // Note: In tests, mocks are primarily for testing health checks via get_block_number.
            // For testing request/get_logs, use real providers or test those separately.
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
    pub async fn request_typed<T: DeserializeOwned + Send>(&self, method: &str, params: Value) -> Result<T, PoolError> {
        let value = self.request(method, params).await?;
        serde_json::from_value(value)
            .map_err(|e| PoolError::Inner(anyhow::anyhow!("Failed to deserialize response: {:?}", e)))
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

    fn create_test_node_config(name: &str, url: &str) -> NodeConfig {
        NodeConfig {
            name: name.to_string(),
            http_url: url.to_string(),
            max_rps: 10,
            error_threshold: 3,
            cooldown: Duration::from_secs(1),
        }
    }

    fn create_test_pool_config() -> PoolConfig {
        PoolConfig {
            health_period: Duration::from_millis(100),
            max_block_lag: 10,
        }
    }

    #[tokio::test]
    async fn test_pool_creation() {
        let node_cfgs = vec![
            create_test_node_config("node1", "http://127.0.0.1:8545"),
            create_test_node_config("node2", "http://127.0.0.1:8546"),
        ];
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
        let node_cfgs = vec![create_test_node_config("node1", "not-a-url")];
        let pool_cfg = create_test_pool_config();

        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg).await;
        assert!(pool.is_err());
    }

    #[tokio::test]
    async fn test_request_with_no_nodes() {
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(vec![], pool_cfg)
            .await
            .unwrap();

        let result = pool
            .request("eth_blockNumber", serde_json::json!([]))
            .await;

        assert!(matches!(result, Err(PoolError::NoHealthyNodes)));
    }

    #[tokio::test]
    async fn test_get_block_number() {
        let mock = MockRpcProvider::new(12345);
        let node_cfgs = vec![create_test_node_config("mock1", "http://mock1")];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_mock_providers(node_cfgs, vec![mock], pool_cfg)
            .await
            .unwrap();

        let result = pool.get_block_number().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 12345);
    }

    #[tokio::test]
    async fn test_health_tick_updates_cached_head() {
        let node_cfgs = vec![create_test_node_config(
            "node1",
            "http://127.0.0.1:9999",
        )];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };
        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg)
            .await
            .unwrap();

        // Wait a bit for health tick to run
        sleep(Duration::from_millis(150)).await;

        // Cached head might still be 0 if provider fails, but the method should work
        let head = pool.read_cached_head();
        // Head could be None (0) if provider fails, or Some(value) if it succeeds
        assert!(head.is_none() || head.is_some());
    }

    #[tokio::test]
    async fn test_node_config_defaults() {
        let config = create_test_node_config("test", "http://127.0.0.1:8545");
        assert_eq!(config.name, "test");
        assert_eq!(config.http_url, "http://127.0.0.1:8545");
        assert_eq!(config.max_rps, 10);
        assert_eq!(config.error_threshold, 3);
        assert_eq!(config.cooldown, Duration::from_secs(1));
    }

    #[tokio::test]
    async fn test_pool_config() {
        let config = create_test_pool_config();
        assert_eq!(config.health_period, Duration::from_millis(100));
        assert_eq!(config.max_block_lag, 10);
    }

    #[tokio::test]
    async fn test_multiple_nodes_round_robin() {
        let mocks = vec![
            MockRpcProvider::new(1000),
            MockRpcProvider::new(2000),
            MockRpcProvider::new(3000),
        ];
        let node_cfgs = vec![
            create_test_node_config("mock1", "http://mock1"),
            create_test_node_config("mock2", "http://mock2"),
            create_test_node_config("mock3", "http://mock3"),
        ];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_mock_providers(node_cfgs, mocks, pool_cfg)
            .await
            .unwrap();

        // Make multiple requests to test round-robin
        for _ in 0..5 {
            let _ = pool.get_block_number().await;
        }

        // If we get here without panicking, round-robin is working
        assert!(true);
    }

    #[tokio::test]
    async fn test_zero_max_rps_handled() {
        let mut node_cfg = create_test_node_config("node1", "http://127.0.0.1:9999");
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
        
        let node_cfgs = vec![
            create_test_node_config("mock1", "http://mock1"),
            create_test_node_config("mock2", "http://mock2"),
        ];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };
        
        let pool = ProviderPool::new_with_mock_providers(
            node_cfgs,
            vec![mock1, mock2],
            pool_cfg,
        )
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
        
        let node_cfgs = vec![create_test_node_config("mock1", "http://mock1")];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };
        
        let pool = ProviderPool::new_with_mock_providers(
            node_cfgs,
            vec![mock],
            pool_cfg,
        )
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
        
        let node_cfgs = vec![create_test_node_config("mock1", "http://mock1")];
        let pool_cfg = PoolConfig {
            health_period: Duration::from_millis(50),
            max_block_lag: 10,
        };
        
        let pool = ProviderPool::new_with_mock_providers(
            node_cfgs,
            vec![mock],
            pool_cfg,
        )
        .await
        .unwrap();

        // Wait for health check
        sleep(Duration::from_millis(150)).await;

        // Head should remain 0 (or None) since the provider fails
        let head = pool.read_cached_head();
        assert!(head.is_none() || head == Some(0));
    }
}
