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
use futures::future;
use parking_lot::RwLock;
use rand::seq::IndexedRandom;
use serde_json::Value;
use thiserror::Error;
use tokio::time::sleep;

use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, direct::NotKeyed},
};

use alloy::{
    network::Ethereum, primitives::{Address, FixedBytes},
    providers::{DynProvider, Provider, ProviderBuilder, layers::CallBatchLayer},
    rpc::types::{Filter, Log}
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

    // Multicall3-based batching
    // be aware on this field: it reduces RPS to the node by the factor of (1 / batching_wait_period)
    // 0 means Multicall3 batching is disabled
    pub multicall_batching_wait: Duration, // batch collecting period (CallBatchLayer) in microseconds
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

                let provider = if nc.multicall_batching_wait > Duration::ZERO {
                    ProviderBuilder::new()
                        .layer(CallBatchLayer::new().wait(nc.multicall_batching_wait))
                        .connect_http(url)
                        .erased()
                } else {
                    ProviderBuilder::new()
                        .connect_http(url)
                        .erased()
                };

                let limiter = RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(nc.max_rps.max(1)).unwrap(),
                ));

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
                let limiter = RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(nc.max_rps.max(1)).unwrap(),
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
        if v == 0 { None } else { Some(v) }
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

                if self.nodes.len() > 1 {
                    let next_node = &self.nodes[*primary];
                    tracing::warn!(
                        "Primary node {} has been cooldowned too many times. Switching primary to {}.",
                        node.cfg.name,
                        next_node.cfg.name
                    );
                }
            }
        }
    }

    /// Mark node success (clear errors/cooldown).
    fn mark_ok(&self, node: &Arc<Node>) {
        let mut st = node.state.write();
        st.consecutive_errors = 0;
        st.disabled_until = None;
    }

    /// Pick the first available node (healthy and not rate-limited), starting from the primary one.
    /// If ignore_limiter is true, randomly selects from all available nodes.
    fn pick_node(&self, ignore_limiter: bool) -> Option<Arc<Node>> {
        let len = self.nodes.len();
        if len == 0 {
            // don't look for anything in a vacuum :)
            return None;
        }

        if !ignore_limiter {
            // Sequential selection starting from primary node
            let start = *self.primary_idx.read();

            // return None if there are no available and non-saturated nodes
            (0..len)
                .map(|i| &self.nodes[(start + i) % len])
                .find(|node| self.is_node_available(node) && node.limiter.check().is_ok())
                .cloned()
        } else {
            // If we should ignore he rate limiter, we need to pick a random node from the available ones
            // to balance the load in case of all nodes are saturated
            let available_nodes: Vec<_> = self
                .nodes
                .iter()
                .filter(|node| self.is_node_available(node))
                .collect();

            available_nodes
                .choose(&mut rand::rng())
                .map(|arc_node| Arc::clone(arc_node))
        }
    }

    /// Generic provider operation executor with retry logic, error handling, and failover.
    pub async fn execute_provider_operation<F, Fut, T>(&self, operation: F) -> Result<T, PoolError>
    where
        F: Fn(DynProvider<Ethereum>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        if self.nodes.is_empty() {
            return Err(PoolError::NoHealthyNodes);
        }

        const PICK_NODE_DELAY_MS: u64 = 10;

        let mut attempt: u32 = 0;

        while attempt <= self.cfg.retry_count {
            // Try to get a node that is available and not rate-limited
            if let Some(node) = self.pick_node(false) {
                if let Some(result) = self.run_on_node(node, &operation, &mut attempt).await? {
                    return Ok(result);
                }

                // run_on_node already bumped `attempt` and applied backoff if needed.
                continue;
            }

            // Try to get any healthy node ignoring the rate limiter
            // it's a case when all nodes are saturated => we should wait when node becomes ready
            if let Some(node) = self.pick_node(true) {
                // wait until the node is ready to accept requests
                node.limiter.until_ready().await;

                if let Some(result) = self.run_on_node(node, &operation, &mut attempt).await? {
                    return Ok(result);
                }
            }

            tokio::time::sleep(Duration::from_millis(PICK_NODE_DELAY_MS)).await;
        }

        Err(PoolError::NoHealthyNodes)
    }

    /// Internal helper: run operation on a given node, update node state, handle retries & backoff.
    async fn run_on_node<F, Fut, T>(
        &self,
        node: Arc<Node>,
        operation: &F,
        attempt: &mut u32,
    ) -> Result<Option<T>, PoolError>
    where
        F: Fn(DynProvider<Ethereum>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        let provider = node.provider.clone();

        match operation(provider).await {
            Ok(result) => {
                self.mark_ok(&node);
                Ok(Some(result))
            }
            Err(e) => {
                self.mark_error(&node);

                if *attempt >= self.cfg.retry_count {
                    return Err(PoolError::Inner(e.into()));
                }

                *attempt += 1;

                // Exponential backoff with cap.
                let delay_ms = (self.cfg.retry_initial_delay_ms * (1u64 << (*attempt - 1).min(5)))
                    .min(self.cfg.retry_max_delay_ms);

                tokio::time::sleep(Duration::from_millis(delay_ms)).await;

                Ok(None)
            }
        }
    }

    /// Making arbitrary JSON-RPC request
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, PoolError> {
        let method = method.to_string();
        let params_raw = serde_json::value::to_raw_value(&params)
            .map_err(|e| anyhow::anyhow!("Failed to serialize params: {:?}", e))?;

        self.execute_provider_operation(move |provider| {
            let method = method.clone();
            let params_raw = params_raw.clone();
            async move {
                let response = provider
                    .raw_request_dyn(method.into(), &params_raw)
                    .await
                    .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))?;
                serde_json::from_str(response.get())
                    .map_err(|e| anyhow::anyhow!("Failed to deserialize response: {:?}", e))
            }
        })
        .await
    }

    /// Get the current block number from a healthy node.
    pub async fn get_block_number(&self) -> Result<u64, PoolError> {
        self.execute_provider_operation(|provider| async move {
            provider
                .get_block_number()
                .await
                .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))
        })
        .await
    }

    // Get logs with filtering by block number, address, and event
    // `event` should be a normalized event name (e.g. 'Transfer(address,address,uint256)')
    // or a valid event signature (32-bytes hex string, 0x-prefixed optionally)
    pub async fn get_logs(
        &self,
        from_block: u64,
        to_block: Option<u64>,
        address: Option<Address>,
        event: Option<String>,
    ) -> Result<Vec<Log>, PoolError> {
        let mut filter = Filter::new().from_block(from_block);
        if let Some(to_block) = to_block {
            filter = filter.to_block(to_block);
        }
        if let Some(address) = address {
            filter = filter.address(address);
        }
        if let Some(event) = &event {
            if let Ok(topic) = event.parse::<FixedBytes<32>>() {
                filter = filter.event_signature(topic);
            } else {
                filter = filter.event(event.as_str());
            }
        }
        
        self.execute_provider_operation(move |provider| {
            let filter = filter.clone(); // Move filter into closure scope
            async move {
                provider.get_logs(&filter)
                    .await
                    .map_err(|e| anyhow::anyhow!("Provider error: {:?}", e))
            }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{providers, providers::mock::Asserter};
    use futures::future;
    use rstest::rstest;
    use std::time::Duration;

    use crate::test_utils::{
        create_mock_provider_with_asserter, create_pool_with_mock_providers, create_test_node,
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

        let pool = create_pool_with_mock_providers(node_cfgs, vec![provider1, provider2], pool_cfg)
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

    #[rstest]
    #[case(1, 5, 2)]
    #[case(3, 21, 10)]
    #[case(3, 30, 10)]
    #[case(3, 300, 30)]
    #[case(10, 1000, 50)]
    #[tokio::test]
    async fn test_load_balancing(
        #[case] num_nodes: usize,
        #[case] num_requests: usize,
        #[case] rps: u32,
    ) {
        let node_cfgs: Vec<_> = (0..num_nodes)
            .map(|i| {
                let mut cfg = create_test_node_with_id(i as u32);
                cfg.max_rps = rps;
                cfg
            })
            .collect();
        let (asserters, providers): (Vec<Asserter>, Vec<providers::DynProvider<Ethereum>>) = (0
            ..num_nodes)
            .map(|_| create_mock_provider_with_asserter())
            .unzip();
        let mut pool_cfg = create_test_pool_config();
        pool_cfg.health_period = Duration::from_secs(100);

        let bns = (1..num_nodes + 1).map(|i| i * 1000).collect::<Vec<_>>();

        // For simplicity, push a bunch of successes for each provider so both
        // health ticks and get_block_number() calls have enough data.
        // Push more than num_requests to account for health ticks and potential retries.
        (0..num_requests * 2).for_each(|_| {
            asserters.iter().enumerate().for_each(|(i, asserter)| {
                asserter.push_success(&bns[i]);
            });
        });

        let pool = create_pool_with_mock_providers(node_cfgs, providers, pool_cfg)
            .await
            .unwrap();

        // Make multiple requests in parallel to hit rate limiter and load balancing.
        let requests: Vec<_> = (0..num_requests).map(|_| pool.get_block_number()).collect();
        let results = future::join_all(requests).await;

        let block_numbers: Vec<u64> = results
            .into_iter()
            .map(|r| r.expect("Request should succeed"))
            .collect();

        let unique_values: std::collections::HashSet<u64> = block_numbers.iter().copied().collect();

        // Compute how many unique nodes should have been hit
        // (at most, the number of nodes; but if not enough requests, could be less)
        // Use integers to avoid f32 Ord issue
        let expected_nodes_used =
            ((num_requests as f32 / rps as f32).ceil() as usize).min(num_nodes);

        assert!(
            unique_values.len() == expected_nodes_used,
            "Expected responses from {} unique nodes. Got: {:?}",
            expected_nodes_used,
            block_numbers
        );
    }
}
