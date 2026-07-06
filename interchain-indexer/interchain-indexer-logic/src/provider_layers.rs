// SPDX-License-Identifier: LicenseRef-Blockscout

use std::{
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    task::{Context, Poll},
    time::{Duration, Instant},
};

use alloy::{
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::{
        TransportError, TransportErrorKind, TransportFut,
        http::{
            Http,
            reqwest::{Client, Url},
        },
    },
};
use alloy_json_rpc::{Request, RequestPacket, ResponsePacket};
use anyhow::{Context as _, Result};
use futures::future;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, direct::NotKeyed},
};
use parking_lot::RwLock;
use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use tokio::time::sleep;
use tower::{Layer, Service, ServiceBuilder};

/// Per-node static config describing a transport endpoint.
#[derive(Clone)]
pub struct NodeConfig {
    pub name: String,
    pub http_url: String,
    pub max_rps: u32,
    pub error_threshold: u32,
    pub cooldown_threshold: u32,
    pub cooldown: Duration,
    pub multicall_batching_wait: Duration,
}

/// Global configuration for the layered transport stack.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct PoolConfig {
    #[serde_as(as = "serde_with::DurationMilliSeconds<u64>")]
    pub health_period: Duration,
    pub max_block_lag: u64,
    pub retry_count: u32,
    pub retry_initial_delay_ms: u64,
    pub retry_max_delay_ms: u64,
}

const RPC_HTTP_TIMEOUT: Duration = Duration::from_secs(30);

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            health_period: Duration::from_millis(1000),
            max_block_lag: 100,
            retry_count: 5,
            retry_initial_delay_ms: 200,
            retry_max_delay_ms: 5000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub struct NodeDefaultSettings {
    pub cooldown_threshold: u32,
    pub cooldown_secs: u64,
}

impl Default for NodeDefaultSettings {
    fn default() -> Self {
        Self {
            cooldown_threshold: 1,
            cooldown_secs: 60,
        }
    }
}

/// Build a provider backed by real HTTP transports and the layered failover stack.
pub fn build_layered_http_provider(
    node_cfgs: Vec<NodeConfig>,
    cfg: PoolConfig,
) -> Result<DynProvider<Ethereum>> {
    let mut transports = Vec::with_capacity(node_cfgs.len());
    let client = Client::builder()
        .timeout(RPC_HTTP_TIMEOUT)
        .build()
        .context("failed to build RPC HTTP client")?;

    for cfg in &node_cfgs {
        let url = Url::parse(&cfg.http_url)?;
        transports.push(Http::with_client(client.clone(), url));
    }

    Ok(build_layered_provider_from_services(
        transports, node_cfgs, cfg,
    ))
}

/// Build a provider from arbitrary transport services.
pub fn build_layered_provider_from_services<S>(
    transports: Vec<S>,
    node_cfgs: Vec<NodeConfig>,
    cfg: PoolConfig,
) -> DynProvider<Ethereum>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Send
        + Sync
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    let layer = ProviderLayer::new(node_cfgs, cfg);
    let transport = ServiceBuilder::new().layer(layer).service(transports);
    let client = RpcClient::builder().transport(transport, false);

    ProviderBuilder::new().connect_client(client).erased()
}

#[derive(Clone)]
struct ProviderLayer {
    node_configs: Arc<[NodeConfig]>,
    config: PoolConfig,
}

impl ProviderLayer {
    fn new(node_configs: Vec<NodeConfig>, config: PoolConfig) -> Self {
        Self {
            node_configs: node_configs.into(),
            config,
        }
    }
}

impl<S> Layer<Vec<S>> for ProviderLayer
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Send
        + Sync
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Service = MultiNodeService<S>;

    fn layer(&self, transports: Vec<S>) -> Self::Service {
        if transports.len() != self.node_configs.len() {
            panic!(
                "provider layer received {} transports but {} node configs",
                transports.len(),
                self.node_configs.len()
            );
        }

        let nodes = transports
            .into_iter()
            .zip(self.node_configs.iter().cloned())
            .enumerate()
            .map(|(index, (service, cfg))| {
                let limiter = RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(cfg.max_rps.max(1)).unwrap(),
                ));

                Arc::new(Node {
                    index,
                    config: cfg,
                    service,
                    limiter,
                    state: RwLock::new(NodeState::default()),
                })
            })
            .collect();

        let pool = Arc::new(PoolState {
            nodes,
            config: self.config.clone(),
            primary_index: RwLock::new(0),
            cached_head: AtomicU64::new(0),
        });

        PoolState::spawn_health_task(pool.clone());

        MultiNodeService { state: pool }
    }
}

#[derive(Clone)]
pub struct MultiNodeService<S> {
    state: Arc<PoolState<S>>,
}

impl<S> Service<RequestPacket> for MultiNodeService<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Send
        + Sync
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: RequestPacket) -> Self::Future {
        let state = self.state.clone();
        Box::pin(async move { state.execute(req).await })
    }
}

struct PoolState<S> {
    nodes: Vec<Arc<Node<S>>>,
    config: PoolConfig,
    primary_index: RwLock<usize>,
    cached_head: AtomicU64,
}

struct Node<S> {
    /// Position of this node in `PoolState::nodes`; used to compare against the
    /// current primary when deciding whether to rotate.
    index: usize,
    config: NodeConfig,
    service: S,
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
    state: RwLock<NodeState>,
}

#[derive(Clone, Copy, Debug, Default)]
struct NodeState {
    disabled_until: Option<Instant>,
    consecutive_errors: u32,
    cooldowns_count: u32,
    last_block: Option<u64>,
    last_block_ts: Option<Instant>,
}

impl<S> PoolState<S>
where
    S: Service<RequestPacket, Response = ResponsePacket, Error = TransportError>
        + Send
        + Sync
        + Clone
        + 'static,
    S::Future: Send + 'static,
{
    async fn execute(&self, req: RequestPacket) -> Result<ResponsePacket, TransportError> {
        if self.nodes.is_empty() {
            return Err(TransportErrorKind::custom_str("no transports configured"));
        }

        const PICK_NODE_DELAY_MS: u64 = 10;
        let mut attempt = 0u32;
        let mut last_error: Option<TransportError> = None;

        while attempt <= self.config.retry_count {
            if let Some(node) = self.pick_node(false) {
                match self.dispatch(node.clone(), req.clone(), false).await {
                    Ok(response) => return Ok(response),
                    Err(err) => {
                        last_error = Some(err);
                        if attempt == self.config.retry_count {
                            break;
                        }
                        attempt += 1;
                        self.apply_backoff(attempt).await;
                        continue;
                    }
                }
            }

            if let Some(node) = self.pick_node(true) {
                match self.dispatch(node.clone(), req.clone(), true).await {
                    Ok(response) => return Ok(response),
                    Err(err) => {
                        last_error = Some(err);
                        if attempt == self.config.retry_count {
                            break;
                        }
                        attempt += 1;
                        self.apply_backoff(attempt).await;
                        continue;
                    }
                }
            }

            sleep(Duration::from_millis(PICK_NODE_DELAY_MS)).await;
        }

        Err(last_error.unwrap_or_else(|| TransportErrorKind::custom_str("no healthy nodes")))
    }

    fn pick_node(&self, ignore_limiter: bool) -> Option<Arc<Node<S>>> {
        let len = self.nodes.len();
        if len == 0 {
            return None;
        }

        if !ignore_limiter {
            let start = *self.primary_index.read();
            (0..len)
                .map(|i| &self.nodes[(start + i) % len])
                .find(|node| self.is_node_available(node) && node.limiter.check().is_ok())
                .cloned()
        } else {
            let available: Vec<_> = self
                .nodes
                .iter()
                .filter(|node| self.is_node_available(node))
                .collect();
            available
                .choose(&mut rand::rng())
                .map(|node| Arc::clone(node))
        }
    }

    async fn dispatch(
        &self,
        node: Arc<Node<S>>,
        req: RequestPacket,
        ignore_limiter: bool,
    ) -> Result<ResponsePacket, TransportError> {
        if ignore_limiter {
            node.limiter.until_ready().await;
        }

        let mut service = node.service.clone();
        let result = match service.call(req).await {
            // A transport-level `Ok` can still carry an embedded JSON-RPC error
            // payload: nodes routinely return auth/rate-limit/server errors as
            // HTTP 200 with an `error` field, which alloy reports as `Ok`.
            // Treat node-health failures as errors so failover and cooldown kick
            // in; otherwise we would keep hammering a broken endpoint forever.
            Ok(response) => failover_error(&response).map_or(Ok(response), Err),
            Err(err) => Err(err),
        };

        match result {
            Ok(response) => {
                self.mark_ok(&node);
                Ok(response)
            }
            Err(err) => {
                // Log the underlying failure so an operator can see *why* a node
                // was cooled down / the primary rotated, instead of only seeing
                // the rotation warning. `fallback` distinguishes a primary
                // request from a random fallback pick.
                tracing::warn!(
                    node = %node.config.name,
                    fallback = ignore_limiter,
                    err = ?err,
                    "RPC request to node failed"
                );
                self.mark_error(&node);
                Err(err)
            }
        }
    }

    fn is_node_available(&self, node: &Node<S>) -> bool {
        let state = *node.state.read();

        if let Some(until) = state.disabled_until
            && Instant::now() < until
        {
            false
        } else if let (Some(node_block), Some(head)) = (state.last_block, self.read_cached_head())
            && head > node_block
            && head - node_block > self.config.max_block_lag
        {
            false
        } else {
            true
        }
    }

    fn read_cached_head(&self) -> Option<u64> {
        let v = self.cached_head.load(Ordering::Relaxed);
        if v == 0 { None } else { Some(v) }
    }

    fn mark_ok(&self, node: &Arc<Node<S>>) {
        let mut state = node.state.write();
        state.consecutive_errors = 0;
        state.disabled_until = None;
    }

    fn mark_error(&self, node: &Arc<Node<S>>) {
        let mut state = node.state.write();
        state.consecutive_errors += 1;
        if state.consecutive_errors >= node.config.error_threshold {
            state.disabled_until = Some(Instant::now() + node.config.cooldown);
            state.consecutive_errors = 0;
            state.cooldowns_count += 1;

            // Only rotate when the *primary* itself keeps failing. A fallback
            // node failing must not move the pointer: a permanently broken
            // endpoint (e.g. an unauthenticated one) is hit constantly via the
            // random fallback path, and advancing the primary on each of its
            // failures churns the pointer and can cycle it right back onto the
            // broken node.
            if state.cooldowns_count >= node.config.cooldown_threshold && self.nodes.len() > 1 {
                let mut primary = self.primary_index.write();
                if *primary == node.index
                    && let Some(new_primary) = self.next_available_node(*primary)
                {
                    tracing::warn!(
                        from = %node.config.name,
                        to = %self.nodes[new_primary].config.name,
                        cooldowns = state.cooldowns_count,
                        "Rotating primary RPC node after repeated failures"
                    );
                    *primary = new_primary;
                    // Reset so a future stint as primary starts with a clean
                    // slate instead of rotating away on its first cooldown.
                    state.cooldowns_count = 0;
                }
            }
        }
    }

    /// Find the next node after `current` (wrapping) that is currently
    /// available, so we don't rotate the primary onto a node that is itself in
    /// cooldown. Never inspects `current` itself, avoiding re-entrant locking
    /// when the caller already holds that node's state lock. Falls back to the
    /// immediate next node so we always rotate off a failing primary.
    fn next_available_node(&self, current: usize) -> Option<usize> {
        let len = self.nodes.len();
        if len <= 1 {
            return None;
        }
        (1..len)
            .map(|offset| (current + offset) % len)
            .find(|&idx| self.is_node_available(&self.nodes[idx]))
            .or(Some((current + 1) % len))
    }

    async fn apply_backoff(&self, attempt: u32) {
        if attempt == 0 {
            return;
        }
        let capped_shift = (attempt - 1).min(5);
        let delay = (self.config.retry_initial_delay_ms * (1u64 << capped_shift))
            .min(self.config.retry_max_delay_ms);
        sleep(Duration::from_millis(delay)).await;
    }

    fn spawn_health_task(state: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                state.health_tick().await;
                sleep(state.config.health_period).await;
            }
        });
    }

    async fn health_tick(&self) {
        let now = Instant::now();
        let initial_head = self.cached_head.load(Ordering::Relaxed);

        let mut probe_nodes = Vec::new();
        for node in &self.nodes {
            let mut state = node.state.write();
            if let Some(until) = state.disabled_until
                && now >= until
            {
                state.disabled_until = None;
                state.consecutive_errors = 0;
            }

            if state.disabled_until.is_none() {
                probe_nodes.push(Arc::clone(node));
            }
        }

        let requests = probe_nodes.into_iter().map(|node| {
            let mut service = node.service.clone();
            async move {
                let response = service.call(block_number_packet()).await;
                (node, response)
            }
        });

        let results = future::join_all(requests).await;

        let mut max_head = initial_head;
        for (node, result) in results {
            if let Ok(response) = result
                && let Some(block_number) = decode_block_number(response)
            {
                let mut state = node.state.write();
                state.last_block = Some(block_number);
                state.last_block_ts = Some(now);
                max_head = max_head.max(block_number);
            }
        }

        self.cached_head.store(max_head, Ordering::Relaxed);
    }
}

fn block_number_packet() -> RequestPacket {
    let request = Request::new("eth_blockNumber", 1_u64.into(), ());
    RequestPacket::from(request.serialize().expect("serialize block number request"))
}

fn decode_block_number(response: ResponsePacket) -> Option<u64> {
    match response {
        ResponsePacket::Single(res) => {
            let payload = res.payload.try_into_success().ok()?;
            let hex = serde_json::from_str::<String>(payload.get()).ok()?;
            let trimmed = hex.trim_start_matches("0x");
            u64::from_str_radix(trimmed, 16).ok()
        }
        _ => None,
    }
}

/// Inspect a successful transport response for an embedded JSON-RPC error that
/// should be treated as a node failure (triggering failover + cooldown).
///
/// Returns the equivalent [`TransportError`] for the first such error payload,
/// or `None` when the response is genuinely successful or only carries errors
/// that are not the node's fault (see [`should_failover_on_error`]).
fn failover_error(packet: &ResponsePacket) -> Option<TransportError> {
    packet.iter_errors().find_map(|err| {
        should_failover_on_error(err.code, err.message.as_ref())
            .then(|| TransportError::ErrorResp(err.clone()))
    })
}

/// Decide whether a JSON-RPC error response indicates an unhealthy node and so
/// should trigger failover.
///
/// Node-health problems — auth failures, rate limiting, server overload,
/// internal errors — should fail over. Deterministic request errors fail
/// identically on every endpoint, so failing over only wastes attempts and
/// wrongly penalizes healthy nodes; those propagate to the caller unchanged.
fn should_failover_on_error(code: i64, message: &str) -> bool {
    match () {
        _ if is_node_health_error(message) => true,
        _ if is_request_deterministic_error(code) => false,
        _ if is_benign_server_error(message) => false,
        _ => true,
    }
}

/// JSON-RPC error codes that reflect the *request* or its on-chain outcome
/// rather than node health, so they fail identically on every endpoint.
/// Includes the standard malformed-request codes and `3` ("execution reverted"),
/// which is a normal result of an `eth_call`/`eth_estimateGas` that reverts.
fn is_request_deterministic_error(code: i64) -> bool {
    const EXECUTION_REVERTED: i64 = 3;
    const PARSE_ERROR: i64 = -32700;
    const INVALID_REQUEST: i64 = -32600;
    const METHOD_NOT_FOUND: i64 = -32601;
    const INVALID_PARAMS: i64 = -32602;
    matches!(
        code,
        EXECUTION_REVERTED | PARSE_ERROR | INVALID_REQUEST | METHOD_NOT_FOUND | INVALID_PARAMS
    )
}

/// Explicit node-health conditions some nodes report with deterministic
/// JSON-RPC codes. The message should override the code because these failures
/// are endpoint-specific and can succeed on another node.
fn is_node_health_error(message: &str) -> bool {
    const NODE_HEALTH: [&str; 8] = [
        "unauthorized",
        "unauthenticated",
        "forbidden",
        "authenticate",
        "api key",
        "rate limit",
        "too many requests",
        "quota exceeded",
    ];
    let message = message.to_ascii_lowercase();
    NODE_HEALTH.iter().any(|pattern| message.contains(pattern))
}

/// Benign, request-specific conditions some nodes report in the implementation-
/// defined server-error range (e.g. querying past the chain head, or a contract
/// call that reverts — some clients surface these under `-32000`/`-32015`
/// instead of a dedicated code). These are not node-health problems and are
/// already handled gracefully by callers, so they should not trigger failover.
fn is_benign_server_error(message: &str) -> bool {
    const BENIGN: [&str; 2] = [
        "from block is greater than latest block",
        "execution reverted",
    ];
    let message = message.to_ascii_lowercase();
    BENIGN.iter().any(|pattern| message.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::Provider;
    use alloy_json_rpc::{ErrorPayload, Id, Response, ResponsePayload};
    use parking_lot::Mutex;
    use serde_json::value::to_raw_value;
    use std::{collections::VecDeque, future};

    #[derive(Clone)]
    struct MockRpcService {
        actions: Arc<Mutex<VecDeque<MockAction>>>,
    }

    enum MockAction {
        Success(u64),
        Error(&'static str),
        /// HTTP-200 response carrying a JSON-RPC error payload.
        ErrorPayload {
            code: i64,
            message: &'static str,
        },
    }

    impl MockRpcService {
        fn new() -> Self {
            Self {
                actions: Arc::new(Mutex::new(VecDeque::new())),
            }
        }

        fn push_success(&self, block: u64) {
            self.actions.lock().push_back(MockAction::Success(block));
        }

        fn push_error(&self, msg: &'static str) {
            self.actions.lock().push_back(MockAction::Error(msg));
        }

        fn push_error_payload(&self, code: i64, message: &'static str) {
            self.actions
                .lock()
                .push_back(MockAction::ErrorPayload { code, message });
        }
    }

    impl Service<RequestPacket> for MockRpcService {
        type Response = ResponsePacket;
        type Error = TransportError;
        type Future = TransportFut<'static>;

        fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }

        fn call(&mut self, req: RequestPacket) -> Self::Future {
            let action = self
                .actions
                .lock()
                .pop_front()
                .unwrap_or(MockAction::Error("no mock action queued"));

            let result = match action {
                MockAction::Success(block) => Ok(build_mock_response(&req, block)),
                MockAction::Error(msg) => Err(TransportErrorKind::custom_str(msg)),
                MockAction::ErrorPayload { code, message } => {
                    Ok(build_mock_error_response(&req, code, message))
                }
            };

            Box::pin(future::ready(result))
        }
    }

    fn build_mock_response(req: &RequestPacket, block: u64) -> ResponsePacket {
        let id = req
            .as_single()
            .map(|serialized| serialized.meta().id.clone())
            .unwrap_or_else(|| Id::Number(1_u64));

        let payload = to_raw_value(&format!("0x{block:x}")).unwrap();
        let response = Response {
            id,
            payload: ResponsePayload::Success(payload),
        };
        ResponsePacket::Single(response)
    }

    fn build_mock_error_response(
        req: &RequestPacket,
        code: i64,
        message: &'static str,
    ) -> ResponsePacket {
        let id = req
            .as_single()
            .map(|serialized| serialized.meta().id.clone())
            .unwrap_or_else(|| Id::Number(1_u64));

        let response = Response {
            id,
            payload: ResponsePayload::Failure(ErrorPayload {
                code,
                message: std::borrow::Cow::Borrowed(message),
                data: None,
            }),
        };
        ResponsePacket::Single(response)
    }

    fn base_node(id: u32) -> NodeConfig {
        NodeConfig {
            name: format!("node{id}"),
            http_url: format!("http://localhost:{id}"),
            max_rps: 10,
            error_threshold: 1,
            cooldown_threshold: 2,
            cooldown: Duration::from_millis(50),
            multicall_batching_wait: Duration::ZERO,
        }
    }

    fn quiet_pool_cfg() -> PoolConfig {
        PoolConfig {
            health_period: Duration::from_secs(60),
            max_block_lag: 100,
            retry_count: 2,
            retry_initial_delay_ms: 5,
            retry_max_delay_ms: 10,
        }
    }

    #[tokio::test]
    async fn layered_provider_returns_block_number() {
        let service = MockRpcService::new();
        // Health tick + user call.
        service.push_success(42);
        service.push_success(42);

        let provider = build_layered_provider_from_services(
            vec![service],
            vec![base_node(1)],
            quiet_pool_cfg(),
        );

        tokio::time::sleep(Duration::from_millis(20)).await;

        let block = provider.get_block_number().await.expect("block number");
        assert_eq!(block, 42);
    }

    #[tokio::test]
    async fn layered_provider_fails_over_on_error() {
        let node_a = base_node(1);
        let node_b = base_node(2);

        let service_a = MockRpcService::new();
        let service_b = MockRpcService::new();

        // Health ticks.
        service_a.push_success(10);
        service_b.push_success(100);
        // Actual call: node A fails, node B succeeds.
        service_a.push_error("boom");
        service_b.push_success(100);

        let provider = build_layered_provider_from_services(
            vec![service_a, service_b],
            vec![node_a, node_b],
            quiet_pool_cfg(),
        );

        tokio::time::sleep(Duration::from_millis(20)).await;

        let block = provider.get_block_number().await.expect("block number");
        assert_eq!(block, 100);
    }

    #[tokio::test]
    async fn layered_provider_fails_over_on_error_payload() {
        let node_a = base_node(1);
        let node_b = base_node(2);

        let service_a = MockRpcService::new();
        let service_b = MockRpcService::new();

        // Health ticks.
        service_a.push_success(10);
        service_b.push_success(100);
        // Actual call: node A returns an HTTP-200 "Unauthorized" JSON-RPC error
        // payload (as an unauthenticated provider would), node B succeeds.
        service_a.push_error_payload(
            -32000,
            "Unauthorized: You must authenticate your request with an API key.",
        );
        service_b.push_success(100);

        let provider = build_layered_provider_from_services(
            vec![service_a, service_b],
            vec![node_a, node_b],
            quiet_pool_cfg(),
        );

        tokio::time::sleep(Duration::from_millis(20)).await;

        // Without failover on the error payload this would surface the -32000
        // error instead of rotating to the healthy node.
        let block = provider.get_block_number().await.expect("block number");
        assert_eq!(block, 100);
    }

    #[test]
    fn classifies_node_health_errors_for_failover() {
        // Node-health problems → fail over.
        assert!(should_failover_on_error(
            -32000,
            "Unauthorized: You must authenticate your request with an API key."
        ));
        assert!(should_failover_on_error(-32005, "rate limit exceeded"));
        assert!(should_failover_on_error(-32603, "internal error"));
        assert!(should_failover_on_error(
            -32602,
            "Unauthorized: You must authenticate your request with an API key."
        ));
        assert!(should_failover_on_error(-32600, "rate limit exceeded"));

        // Deterministic request errors → do not fail over.
        assert!(!should_failover_on_error(-32602, "invalid params"));
        assert!(!should_failover_on_error(
            -32601,
            "the method does not exist"
        ));

        // Benign server-range condition handled by callers → do not fail over.
        assert!(!should_failover_on_error(
            -32000,
            "from block is greater than latest block"
        ));

        // Contract reverts are a normal call outcome, not a node fault, whether
        // reported under the dedicated code or in the server-error range.
        assert!(!should_failover_on_error(3, "execution reverted"));
        assert!(!should_failover_on_error(
            -32000,
            "execution reverted: ERC20: transfer amount exceeds balance"
        ));
    }

    fn build_test_pool(
        n: usize,
        error_threshold: u32,
        cooldown_threshold: u32,
    ) -> PoolState<MockRpcService> {
        let nodes = (0..n)
            .map(|i| {
                let cfg = NodeConfig {
                    name: format!("node{i}"),
                    http_url: format!("http://localhost:{i}"),
                    max_rps: 10,
                    error_threshold,
                    cooldown_threshold,
                    cooldown: Duration::from_secs(60),
                    multicall_batching_wait: Duration::ZERO,
                };
                Arc::new(Node {
                    index: i,
                    limiter: RateLimiter::direct(Quota::per_second(
                        NonZeroU32::new(cfg.max_rps).unwrap(),
                    )),
                    config: cfg,
                    service: MockRpcService::new(),
                    state: RwLock::new(NodeState::default()),
                })
            })
            .collect();
        PoolState {
            nodes,
            config: quiet_pool_cfg(),
            primary_index: RwLock::new(0),
            cached_head: AtomicU64::new(0),
        }
    }

    #[test]
    fn primary_failure_rotates_to_next_node() {
        let pool = build_test_pool(3, 1, 1);
        let primary = Arc::clone(&pool.nodes[0]);

        pool.mark_error(&primary);

        assert_eq!(
            *pool.primary_index.read(),
            1,
            "primary should rotate off the failing node"
        );
    }

    #[test]
    fn fallback_failure_does_not_rotate_primary() {
        let pool = build_test_pool(3, 1, 1);
        // A non-primary (fallback) node failing — e.g. a permanently broken
        // endpoint hit via the random fallback path — must not churn the
        // primary pointer.
        let fallback = Arc::clone(&pool.nodes[2]);

        pool.mark_error(&fallback);

        assert_eq!(
            *pool.primary_index.read(),
            0,
            "primary must stay put when a fallback node fails"
        );
    }

    #[test]
    fn rotation_skips_nodes_in_cooldown() {
        let pool = build_test_pool(3, 1, 1);
        // The immediate next node is in cooldown, so rotation should skip it.
        pool.nodes[1].state.write().disabled_until = Some(Instant::now() + Duration::from_secs(60));
        let primary = Arc::clone(&pool.nodes[0]);

        pool.mark_error(&primary);

        assert_eq!(
            *pool.primary_index.read(),
            2,
            "rotation should skip the node currently in cooldown"
        );
    }
}
