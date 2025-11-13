use std::{
    collections::HashMap,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

use anyhow::Result;
use futures::future;
use parking_lot::RwLock;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;

use governor::{
    clock::DefaultClock,
    state::{direct::NotKeyed, InMemoryState},
    Quota, RateLimiter,
};

use alloy::{
    providers::{DynProvider, Provider, ProviderBuilder},
   network::Ethereum,
};

/// Per-node static config.
#[derive(Clone)]
pub struct NodeConfig {
    pub name: String,
    pub http_url: String,
    pub max_rps: u32,            // max requests per second for this node
    pub error_threshold: u32,    // consecutive errors before temporary cooldown
    pub cooldown_threshold: u32, // the number of cooldowns before exiting the role of the primary node
    pub cooldown: Duration,      // how long node stays disabled
}

/// Global pool config.
#[derive(Clone)]
pub struct PoolConfig {
    /// How often health task runs (block height refresh, revive attempts, etc.)
    pub health_period: Duration,
    /// Maximum allowed lag vs. observed network head (in blocks). If a node lags more, it's skipped.
    pub max_block_lag: u64,
    /// Number of retries for failed requests (0 = no retries)
    /// Retry logic is implemented on the pool level, not on the node level.
    pub retry_count: u32,
    /// Initial delay for exponential backoff retries (in milliseconds)
    pub retry_initial_delay_ms: u64,
    /// Maximum delay for exponential backoff retries (in milliseconds)
    pub retry_max_delay_ms: u64,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            health_period: Duration::from_millis(1000),
            max_block_lag: 20,
            retry_count: 10,
            retry_initial_delay_ms: 50,
            retry_max_delay_ms: 500,
        }
    }
}

struct Node {
    cfg: NodeConfig,
    /// Type-erased Alloy provider
    provider: DynProvider<Ethereum>,
    /// Per-node rate limiter.
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    /// Health & block snapshot.
    state: RwLock<NodeState>,
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
    /// The first node is primary by default. It can be switched in case of many errors.
    primary_idx: RwLock<usize>,
    /// Cached network head (max of nodes' last_block). Updated only by the health task.
    cached_head: AtomicU64,
}

impl ProviderPool {
    /// Create pool and spawn internal health task (self-contained).
    /// Uses real HTTP providers from the node config URLs.
    pub async fn new_with_config(node_cfgs: Vec<NodeConfig>, cfg: PoolConfig) -> Result<Arc<Self>> {
        let nodes: Vec<Arc<Node>> = node_cfgs
            .into_iter()
            .map(|nc| -> Result<Arc<Node>> {
                let url = nc.http_url.parse()?;

                let provider = ProviderBuilder::new()
                    .connect_http(url)
                    .erased();

                let limiter = RateLimiter::direct(
                    Quota::per_second(NonZeroU32::new(nc.max_rps.max(1)).unwrap()),
                );

                Ok(Arc::new(Node {
                    cfg: nc,
                    provider,
                    limiter,
                    state: RwLock::new(NodeState::default()),
                }))
            })
            .collect::<Result<Vec<_>>>()?;

        let pool = Arc::new(Self {
            nodes,
            cfg,
            primary_idx: RwLock::new(0),
            cached_head: AtomicU64::new(0),
        });

        Self::spawn_health_task(pool.clone());

        Ok(pool)
    }

    /// Helper method to create pool with pre-created Alloy providers.
    /// This is used in tests, where providers are built using `mock::Asserter`.
    #[cfg(any(test, feature = "test-utils"))]
    pub(crate) async fn new_with_providers(
        node_cfgs: Vec<NodeConfig>,
        providers: Vec<DynProvider<Ethereum>>,
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
                let limiter = RateLimiter::direct(
                    Quota::per_second(NonZeroU32::new(nc.max_rps.max(1)).unwrap()),
                );
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

        Self::spawn_health_task(pool.clone());

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

        // Decide which nodes we should probe in this tick.
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

                st.disabled_until.is_none()
            })
            .collect();

        // Prepare futures for nodes that need probing.
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

        let block_number_results = future::join_all(block_number_futures).await;

        let result_map: HashMap<usize, &Result<u64, _>> = indices
            .iter()
            .zip(block_number_results.iter())
            .map(|(&idx, result)| (idx, result))
            .collect();

        let max_head = self
            .nodes
            .iter()
            .zip(should_probe_flags.iter())
            .enumerate()
            .fold(initial_head, |current_max, (idx, (n, &should_probe))| {
                if should_probe {
                    if let Some(Ok(bn)) = result_map.get(&idx) {
                        let bn = *bn;
                        let mut st = n.state.write();
                        st.last_block = Some(bn);
                        st.last_block_ts = Some(now);
                        return current_max.max(bn);
                    }
                }
                current_max
            });

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
        if v == 0 {
            None
        } else {
            Some(v)
        }
    }

    /// Mark node error and apply cooldown if threshold exceeded.
    fn mark_error(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors += 1;
        if st.consecutive_errors >= node.cfg.error_threshold {
            st.disabled_until = Some(Instant::now() + node.cfg.cooldown);
            st.consecutive_errors = 0;
            st.cooldowns_count += 1;

            if st.cooldowns_count >= node.cfg.cooldown_threshold {
                let mut primary = self.primary_idx.write();
                *primary = (*primary + 1) % self.nodes.len();
                tracing::warn!(
                    "Primary node {} has been cooldowned too many times. Switching primary to the next node.",
                    node.cfg.name
                );
            }
        }
    }

    /// Mark node success (clear errors/cooldown).
    fn mark_ok(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors = 0;
        st.disabled_until = None;
    }

    /// Pick the first available node (healthy and not rate-limited), starting from primary.
    fn pick_node(&self) -> Option<Arc<Node>> {
        let len = self.nodes.len();
        if len == 0 {
            return None;
        }

        let start = *self.primary_idx.read();

        (0..len)
            .map(|i| &self.nodes[(start + i) % len])
            .find(|node| self.is_node_available(node) && node.limiter.check().is_ok())
            .cloned()
    }

    /// Generic provider operation executor with retry logic, error handling, and failover.
    async fn execute_provider_operation<F, Fut, T>(&self, operation: F) -> Result<T, PoolError>
    where
        F: Fn(Arc<Node>) -> Fut,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        if self.nodes.is_empty() {
            return Err(PoolError::NoHealthyNodes);
        }

        const PICK_NODE_DELAY_MS: u64 = 10;
        const PICK_NODE_MAX_ATTEMPTS: u32 = 1000;

        let mut operation_attempt: u32 = 0;
        let mut pick_node_attempt: u32 = 0;

        while pick_node_attempt < PICK_NODE_MAX_ATTEMPTS
            && operation_attempt <= self.cfg.retry_count
        {
            let Some(node) = self.pick_node() else {
                sleep(Duration::from_millis(PICK_NODE_DELAY_MS)).await;
                pick_node_attempt += 1;
                continue;
            };

            pick_node_attempt = 0;

            match operation(node.clone()).await {
                Ok(result) => {
                    self.mark_ok(&node);
                    return Ok(result);
                }
                Err(e) => {
                    self.mark_error(&node);

                    if operation_attempt >= self.cfg.retry_count {
                        return Err(PoolError::Inner(e.into()));
                    }

                    let delay_ms = (self.cfg.retry_initial_delay_ms
                        * (1u64 << operation_attempt.min(5)))
                        .min(self.cfg.retry_max_delay_ms);
                    sleep(Duration::from_millis(delay_ms)).await;
                }
            }

            operation_attempt += 1;
        }

        Err(PoolError::NoHealthyNodes)
    }

    /// Making arbitrary JSON-RPC request
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, PoolError> {
        let method = method.to_string();
        let params_raw = serde_json::value::to_raw_value(&params)
            .map_err(|e| anyhow::anyhow!("Failed to serialize params: {:?}", e))?;

        self.execute_provider_operation(|node| {
            let method = method.clone();
            let params_raw = params_raw.clone();
            async move {
                let response = node.provider.raw_request_dyn(method.into(), &params_raw).await
                    .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))?;
                serde_json::from_str(response.get())
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize response: {:?}", e))
            }
        })
        .await
    }

    /// Get the current block number from a healthy node.
    pub async fn get_block_number(&self) -> Result<u64, PoolError> {
        self.execute_provider_operation(|node| async move {
            // Alloy provider's `get_block_number` returns a future-like ProviderCall.
            node.provider
                .get_block_number()
                .await
                .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))
        })
        .await
    }

    // TODO: implement getLogs, call smart contract and other typical operations
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use futures::future;

    use crate::test_utils::{
        create_mock_provider_with_asserter,
        create_pool_with_mock_providers,
        create_test_node,
        create_test_node_with_id,
        create_test_pool_config,
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
        let node_cfgs = vec![create_test_node()];
        let pool_cfg = create_test_pool_config();

        // Create mock provider + asserter
        let (asserter, provider) = create_mock_provider_with_asserter();

        let expected_bn = 12345u64;
        // First response will be used by initial health tick, second by get_block_number().
        asserter.push_success(&expected_bn);
        asserter.push_success(&expected_bn);

        let pool = create_pool_with_mock_providers(node_cfgs, vec![provider], pool_cfg)
            .await
            .unwrap();

        let result = pool.get_block_number().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), expected_bn);
    }

    #[tokio::test]
    async fn test_health_tick_updates_cached_head() {
        let node_cfgs = vec![create_test_node()];
        let pool_cfg = create_test_pool_config();

        let (asserter, provider) = create_mock_provider_with_asserter();

        let bn = 42u64;
        // Give enough responses for several health ticks.
        (0..5).for_each(|_| {
            asserter.push_success(&bn);
        });

        let pool = create_pool_with_mock_providers(node_cfgs, vec![provider], pool_cfg)
            .await
            .unwrap();

        // Wait a bit for health tick to run.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let head = pool.read_cached_head();
        assert_eq!(head, Some(bn));
    }

    #[tokio::test]
    async fn test_multiple_nodes() {
        let node_cfgs = vec![
            create_test_node_with_id(1),
            create_test_node_with_id(2),
            create_test_node_with_id(3),
        ];
        let pool_cfg = create_test_pool_config();

        // Create three mock providers with distinct block numbers.
        let (asserter1, provider1) = create_mock_provider_with_asserter();
        let (asserter2, provider2) = create_mock_provider_with_asserter();
        let (asserter3, provider3) = create_mock_provider_with_asserter();

        let bns = [1000u64, 2000u64, 3000u64];

        // For simplicity, push a bunch of successes for each provider so both
        // health ticks and get_block_number() calls have enough data.
        (0..50).for_each(|_| {
            asserter1.push_success(&bns[0]);
            asserter2.push_success(&bns[1]);
            asserter3.push_success(&bns[2]);
        });

        let pool = create_pool_with_mock_providers(
            node_cfgs,
            vec![provider1, provider2, provider3],
            pool_cfg,
        )
        .await
        .unwrap();

        // Make multiple requests in parallel to hit rate limiter and load balancing.
        let requests: Vec<_> = (0..30).map(|_| pool.get_block_number()).collect();
        let results = future::join_all(requests).await;

        let block_numbers: Vec<u64> = results
            .into_iter()
            .map(|r| r.expect("Request should succeed"))
            .collect();

        let unique_values: std::collections::HashSet<u64> =
            block_numbers.iter().copied().collect();

        assert!(
            unique_values.len() == 3,
            "Expected responses from all three nodes. Got: {:?}",
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
        let node_cfgs = vec![create_test_node_with_id(1), create_test_node_with_id(2)];
        let pool_cfg = create_test_pool_config();

        let (asserter1, provider1) = create_mock_provider_with_asserter();
        let (asserter2, provider2) = create_mock_provider_with_asserter();

        let bn1 = 1000u64;
        let bn2 = 1005u64;

        // Push several responses for each provider.
        (0..5).for_each(|_| {
            asserter1.push_success(&bn1);
            asserter2.push_success(&bn2);
        });

        let pool = create_pool_with_mock_providers(
            node_cfgs,
            vec![provider1, provider2],
            pool_cfg,
        )
        .await
        .unwrap();

        // Wait for health tick to run.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let head = pool.read_cached_head();
        assert!(head.is_some());
        assert_eq!(head.unwrap(), bn2);
    }

    #[tokio::test]
    async fn test_mock_provider_update_block_number() {
        let node_cfgs = vec![create_test_node()];
        let pool_cfg = create_test_pool_config();

        let (asserter, provider) = create_mock_provider_with_asserter();

        // Initially block number = 1000.
        (0..5).for_each(|_| {
            asserter.push_success(&1000u64);
        });

        let pool = create_pool_with_mock_providers(node_cfgs, vec![provider], pool_cfg)
            .await
            .unwrap();

        // Wait for initial health tick.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(pool.read_cached_head(), Some(1000));

        // Later we want block number to become 2000.
        (0..5).for_each(|_| {
            asserter.push_success(&2000u64);
        });

        // Wait for next health tick to pick up new value.
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(pool.read_cached_head(), Some(2000));
    }

    #[tokio::test]
    async fn test_mock_provider_failure() {
        let node_cfgs = vec![create_test_node()];
        let pool_cfg = create_test_pool_config();

        let (asserter, provider) = create_mock_provider_with_asserter();

        // First few calls (health ticks) will fail.
        (0..3).for_each(|_| {
            asserter.push_failure_msg("SOME ERROR");
        });

        let pool = create_pool_with_mock_providers(node_cfgs, vec![provider], pool_cfg)
            .await
            .unwrap();

        // Wait for health tick: all attempts fail, cached head should stay None.
        tokio::time::sleep(Duration::from_millis(150)).await;
        let head = pool.read_cached_head();
        assert!(head.is_none());

        // Now push successful responses.
        (0..5).for_each(|_| {
            asserter.push_success(&1000u64);
        });

        // Wait for next health ticks to succeed.
        tokio::time::sleep(Duration::from_millis(150)).await;

        let head_after_restore = pool.read_cached_head();
        assert_eq!(head_after_restore, Some(1000));

        // Also verify we can get block number directly.
        let block_number = pool.get_block_number().await;
        assert_eq!(block_number.unwrap(), 1000);
    }
}
