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
        http::{Http, reqwest::Url},
    },
};
use alloy_json_rpc::{Request, RequestPacket, ResponsePacket};
use anyhow::Result;
use futures::future;
use governor::{
    Quota, RateLimiter,
    clock::DefaultClock,
    state::{InMemoryState, direct::NotKeyed},
};
use parking_lot::RwLock;
use rand::seq::IndexedRandom;
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
#[derive(Clone)]
pub struct PoolConfig {
    pub health_period: Duration,
    pub max_block_lag: u64,
    pub retry_count: u32,
    pub retry_initial_delay_ms: u64,
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

/// Build a provider backed by real HTTP transports and the layered failover stack.
pub fn build_layered_http_provider(
    node_cfgs: Vec<NodeConfig>,
    cfg: PoolConfig,
) -> Result<DynProvider<Ethereum>> {
    let mut transports = Vec::with_capacity(node_cfgs.len());

    for cfg in &node_cfgs {
        let url = Url::parse(&cfg.http_url)?;
        transports.push(Http::new(url));
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
            .map(|(service, cfg)| {
                let limiter = RateLimiter::direct(Quota::per_second(
                    NonZeroU32::new(cfg.max_rps.max(1)).unwrap(),
                ));

                Arc::new(Node {
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
    config: NodeConfig,
    service: S,
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock>,
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
        match service.call(req).await {
            Ok(response) => {
                self.mark_ok(&node);
                Ok(response)
            }
            Err(err) => {
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

            if state.cooldowns_count >= node.config.cooldown_threshold && self.nodes.len() > 1 {
                let mut primary = self.primary_index.write();
                *primary = (*primary + 1) % self.nodes.len();
            }
        }
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
            if let Some(until) = state.disabled_until {
                if now >= until {
                    state.disabled_until = None;
                    state.consecutive_errors = 0;
                }
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
            if let Ok(response) = result {
                if let Some(block_number) = decode_block_number(response) {
                    let mut state = node.state.write();
                    state.last_block = Some(block_number);
                    state.last_block_ts = Some(now);
                    max_head = max_head.max(block_number);
                }
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
        ResponsePacket::Single(res) => match res.payload.try_into_success().ok()? {
            payload => {
                let hex = serde_json::from_str::<String>(payload.get()).ok()?;
                let trimmed = hex.trim_start_matches("0x");
                u64::from_str_radix(trimmed, 16).ok()
            }
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::providers::Provider;
    use alloy_json_rpc::{Id, Response, ResponsePayload};
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
            };

            Box::pin(future::ready(result))
        }
    }

    fn build_mock_response(req: &RequestPacket, block: u64) -> ResponsePacket {
        let id = req
            .as_single()
            .map(|serialized| serialized.meta().id.clone())
            .unwrap_or_else(|| Id::Number(1_u64.into()));

        let payload = to_raw_value(&format!("0x{block:x}")).unwrap();
        let response = Response {
            id,
            payload: ResponsePayload::Success(payload),
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
}
