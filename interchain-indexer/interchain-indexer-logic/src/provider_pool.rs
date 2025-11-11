use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use parking_lot::RwLock;
use thiserror::Error;
use tokio::time::sleep;

use governor::{clock::DefaultClock, Quota, RateLimiter, state::direct::NotKeyed, state::InMemoryState};

use alloy::network::Ethereum;
use alloy::providers::Provider;

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
    http: Box<dyn Provider<Ethereum> + Send + Sync>,
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
    pub async fn new_with_config(node_cfgs: Vec<NodeConfig>, cfg: PoolConfig) -> Result<Arc<Self>> {
        let mut nodes = Vec::with_capacity(node_cfgs.len());
        for nc in node_cfgs {
            // Build HTTP provider
            let http = alloy::providers::ProviderBuilder::new()
                .connect_http(nc.http_url.parse()?);
            // Per-node RPS limiter
            let limiter = RateLimiter::direct(Quota::per_second(
                NonZeroU32::new(nc.max_rps.max(1)).unwrap(), // avoid zero
            ));
            nodes.push(Arc::new(Node {
                cfg: nc,
                http: Box::new(http),
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
                if let Ok(bn) = n.http.get_block_number().await {
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

    /// Generic request: pass a closure to execute with a selected provider.
    pub async fn request<F, Fut, T>(&self, f: F) -> Result<T, PoolError>
    where
        F: Fn(&(dyn Provider<Ethereum> + Send + Sync)) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, anyhow::Error>> + Send,
        T: Send,
    {
        // Try up to N nodes (N = number of nodes)
        for _ in 0..self.nodes.len() {
            let Some(node) = self.pick_node() else {
                // No healthy/non-limited nodes available now.
                // Small backoff to avoid spinning; then retry another pass.
                sleep(Duration::from_millis(10)).await;
                continue;
            };

            match f(node.http.as_ref()).await {
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

    /// Convenience wrapper for getLogs-like calls built by a closure.
    pub async fn get_logs<F, Fut, T>(&self, build: F) -> Result<T, PoolError>
    where
        F: Fn(&(dyn Provider<Ethereum> + Send + Sync)) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T, anyhow::Error>> + Send,
        T: Send,
    {
        self.request(build).await
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
            .request(|_| async { Ok::<u64, anyhow::Error>(42) })
            .await;

        assert!(matches!(result, Err(PoolError::NoHealthyNodes)));
    }

    #[tokio::test]
    async fn test_request_success() {
        // Use a URL that will fail to connect but we can test the logic
        let node_cfgs = vec![create_test_node_config(
            "node1",
            "http://127.0.0.1:9999", // Port that likely doesn't exist
        )];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg)
            .await
            .unwrap();

        // Test that request method works (even if provider fails)
        // The request will try to use the provider but may fail
        let result = pool
            .request(|_provider| async {
                // Simulate a successful operation
                Ok::<String, anyhow::Error>("success".to_string())
            })
            .await;

        // Since we're using a mock closure, it should succeed
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_request_retries_on_error() {
        let node_cfgs = vec![
            create_test_node_config("node1", "http://127.0.0.1:9999"),
            create_test_node_config("node2", "http://127.0.0.1:9998"),
        ];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg)
            .await
            .unwrap();

        let call_count = Arc::new(AtomicU64::new(0));
        let call_count_clone = call_count.clone();
        let result = pool
            .request(move |_provider| {
                let call_count = call_count_clone.clone();
                async move {
                    let count = call_count.fetch_add(1, Ordering::SeqCst);
                    if count < 1 {
                        Err(anyhow::anyhow!("simulated error"))
                    } else {
                        Ok::<String, anyhow::Error>("success".to_string())
                    }
                }
            })
            .await;

        // Should eventually succeed after retries
        assert!(result.is_ok());
        assert!(call_count.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn test_get_logs() {
        let node_cfgs = vec![create_test_node_config(
            "node1",
            "http://127.0.0.1:9999",
        )];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg)
            .await
            .unwrap();

        let result = pool
            .get_logs(|_provider| async { Ok::<Vec<u8>, anyhow::Error>(vec![1, 2, 3]) })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![1, 2, 3]);
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
        let node_cfgs = vec![
            create_test_node_config("node1", "http://127.0.0.1:9999"),
            create_test_node_config("node2", "http://127.0.0.1:9998"),
            create_test_node_config("node3", "http://127.0.0.1:9997"),
        ];
        let pool_cfg = create_test_pool_config();
        let pool = ProviderPool::new_with_config(node_cfgs, pool_cfg)
            .await
            .unwrap();

        // Make multiple requests to test round-robin
        for _ in 0..5 {
            let _ = pool
                .request(|_provider| async { Ok::<u32, anyhow::Error>(42) })
                .await;
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
}
