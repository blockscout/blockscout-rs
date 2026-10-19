// SPDX-License-Identifier: LicenseRef-Blockscout

use anyhow::Error;
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use models::{
    operations::{OperationIdsApiResponse, Operations},
    profiling::{ProfilingResponse, V1OperationData, V2OperationData},
};
use reqwest::{Client as HttpClient, Method, Request, Response};
use settings::{RpcSettings, StageProfilingMode};
use std::{
    collections::HashMap,
    fmt,
    num::NonZeroU32,
    sync::{Arc, Mutex},
    time::Instant,
};
use tokio::time::{timeout, Duration};
use tracing::Instrument;

pub mod models;
pub mod settings;

type Limiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

#[derive(Clone)]
pub struct Client {
    rpc: RpcSettings,
    http: HttpClient,
    limiter: Arc<Limiter>,
    v2_circuit: Arc<Mutex<V2Circuit>>,
}

#[derive(Debug)]
struct V2Circuit {
    retry_at: Option<Instant>,
    probe_in_flight: bool,
}

#[derive(Debug)]
pub enum ProfilingError {
    LocalRateLimiter,
    Transport(reqwest::Error),
    Http(reqwest::StatusCode),
    EmptyResponse,
    Deserialize(serde_json::Error),
    Build(anyhow::Error),
}

impl fmt::Display for ProfilingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ProfilingError {}

impl ProfilingError {
    pub fn is_v2_fallback_eligible(&self) -> bool {
        match self {
            Self::Transport(error) => {
                error.is_connect() || error.is_timeout() || error.is_request()
            }
            Self::Http(status) => matches!(
                status.as_u16(),
                404 | 405 | 410 | 500 | 501 | 502 | 503 | 504
            ),
            Self::Deserialize(_) | Self::EmptyResponse => true,
            Self::LocalRateLimiter | Self::Build(_) => false,
        }
    }
}

impl Client {
    pub fn new(settings: RpcSettings) -> Self {
        let http = HttpClient::new();

        let quota = Quota::per_second(NonZeroU32::new(settings.request_per_second).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));
        tracing::info!(
            mode = ?settings.stage_profiling_mode,
            v2_probe_interval = ?settings.stage_profiling_v2_probe_interval,
            "Configured Stage Profiler upstream selection"
        );

        Self {
            rpc: settings,
            http,
            limiter,
            v2_circuit: Arc::new(Mutex::new(V2Circuit {
                retry_at: None,
                probe_in_flight: false,
            })),
        }
    }

    pub async fn get_operations(&self, start: u64, end: u64) -> Result<Operations, Error> {
        let mut all_operations = Vec::new();
        let mut offset = 0;

        loop {
            let url = format!(
                "{}/operation-ids?from={}&till={}&offset={}",
                self.url(),
                start,
                end,
                offset
            );
            let request = Request::new(Method::GET, url.parse()?);
            let response = self
                .make_request(request)
                .instrument(tracing::debug_span!("get_operations", url = %url))
                .await?;

            if !response.status().is_success() {
                let status = response.status();
                tracing::error!(%url, status =? status, "Bad response during operation list request");
                return Err(anyhow::anyhow!(
                    "HTTP error {}: {}",
                    status,
                    status.as_str()
                ));
            }

            let text = response.text().await?;

            if text.is_empty() {
                tracing::error!(%url, "Received empty response from operations list");
                break;
            }

            let parsed = match serde_json::from_str::<OperationIdsApiResponse>(&text) {
                Ok(response) => response.response,
                Err(e) => {
                    tracing::error!(%url, err =? e, "Failed to parse operations list response");
                    return Err(e.into());
                }
            };

            let count = parsed.operations.len();
            all_operations.extend(parsed.operations);

            if all_operations.len() >= usize::try_from(parsed.total).unwrap() || count == 0 {
                break;
            }

            offset = all_operations.len();
        }

        Ok(all_operations)
    }

    pub fn stage_profiling_mode(&self) -> StageProfilingMode {
        self.rpc.stage_profiling_mode
    }

    /// Returns false while prefer-v2 is cooling down or another request owns the probe.
    pub fn v2_available_for_direct_request(&self) -> bool {
        if self.rpc.stage_profiling_mode == StageProfilingMode::V2Only {
            return true;
        }
        if self.rpc.stage_profiling_mode == StageProfilingMode::V1Only {
            return false;
        }
        let mut circuit = self.v2_circuit.lock().expect("v2 circuit mutex poisoned");
        match circuit.retry_at {
            None => true,
            Some(retry_at) if Instant::now() >= retry_at && !circuit.probe_in_flight => {
                circuit.probe_in_flight = true;
                tracing::info!("Stage Profiler v2 circuit probe started");
                true
            }
            Some(_) => false,
        }
    }

    pub async fn get_operations_stages(
        &self,
        id: Vec<&str>,
    ) -> Result<ProfilingResponse, ProfilingError> {
        match self.rpc.stage_profiling_mode {
            StageProfilingMode::V1Only => {
                tracing::debug!(source = "v1", mode = "v1_only", "Selected Stage Profiler");
                self.get_operations_stages_v1(id)
                    .await
                    .map(ProfilingResponse::V1)
            }
            StageProfilingMode::V2Only => {
                tracing::debug!(source = "v2", mode = "v2_only", "Selected Stage Profiler");
                self.get_operations_stages_v2(id)
                    .await
                    .map(ProfilingResponse::V2)
            }
            StageProfilingMode::PreferV2 => {
                if !self.v2_available_for_direct_request() {
                    tracing::debug!("Stage Profiler v2 circuit is open; using v1");
                    return self
                        .get_operations_stages_v1(id)
                        .await
                        .map(ProfilingResponse::V1);
                }
                // Circuit state is owned by `get_operations_stages_v2`; this branch
                // only decides where the outcome is routed, so the state machine
                // keeps a single writer.
                match self.get_operations_stages_v2(id.clone()).await {
                    Ok(response) => {
                        tracing::debug!("Selected Stage Profiler v2");
                        Ok(ProfilingResponse::V2(response))
                    }
                    Err(error) if error.is_v2_fallback_eligible() => {
                        tracing::warn!(error = %error, "Stage Profiler v2 unavailable; using v1");
                        self.get_operations_stages_v1(id)
                            .await
                            .map(ProfilingResponse::V1)
                    }
                    Err(error) => Err(error),
                }
            }
        }
    }

    pub async fn get_operations_stages_v1(
        &self,
        id: Vec<&str>,
    ) -> Result<HashMap<String, V1OperationData>, ProfilingError> {
        self.get_profiling("/stage-profiling", id).await
    }

    pub async fn get_operations_stages_v2(
        &self,
        id: Vec<&str>,
    ) -> Result<HashMap<String, V2OperationData>, ProfilingError> {
        let result = self.get_profiling("/v2/stage-profiling", id).await;
        if result.is_ok() {
            self.close_v2_circuit();
        } else if let Err(error) = &result {
            if error.is_v2_fallback_eligible() {
                self.open_v2_circuit(error);
            } else {
                self.defer_failed_probe();
            }
        }
        result
    }

    async fn get_profiling<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        id: Vec<&str>,
    ) -> Result<HashMap<String, T>, ProfilingError> {
        let request_body = serde_json::json!({ "operationIds": id });
        let url = format!("{}{path}", self.url());
        let parsed_url = url
            .parse()
            .map_err(|error| ProfilingError::Build(anyhow::Error::new(error)))?;
        let mut request = Request::new(Method::POST, parsed_url);

        request
            .headers_mut()
            .insert("accept", "application/json".parse().unwrap());
        request
            .headers_mut()
            .insert("Content-Type", "application/json".parse().unwrap());
        request.body_mut().replace(
            serde_json::to_vec(&request_body)
                .map_err(|error| ProfilingError::Build(error.into()))?
                .into(),
        );

        let response = self
            .make_profiling_request(request)
            .instrument(tracing::debug_span!("get_operations_stages", url = url))
            .await?;

        if response.status().is_success() {
            let text = response.text().await.map_err(ProfilingError::Transport)?;
            if text.is_empty() {
                tracing::error!(url, "Received empty response from stage profiling");
                return Err(ProfilingError::EmptyResponse);
            }

            match serde_json::from_str::<ApiResponse<T>>(&text) {
                Ok(response) => Ok(response.response),
                Err(e) => {
                    tracing::error!(url, err =? e, "Failed to parse staging response");
                    Err(ProfilingError::Deserialize(e))
                }
            }
        } else {
            let status = response.status();
            tracing::error!(%url, status =? status, "Bad response during stage profiling request");
            Err(ProfilingError::Http(status))
        }
    }

    fn open_v2_circuit(&self, error: &ProfilingError) {
        let mut circuit = self.v2_circuit.lock().expect("v2 circuit mutex poisoned");
        circuit.retry_at = Some(Instant::now() + self.rpc.stage_profiling_v2_probe_interval);
        circuit.probe_in_flight = false;
        tracing::warn!(
            reason = %error,
            probe_interval = ?self.rpc.stage_profiling_v2_probe_interval,
            "Stage Profiler v2 circuit opened"
        );
    }

    fn close_v2_circuit(&self) {
        let mut circuit = self.v2_circuit.lock().expect("v2 circuit mutex poisoned");
        let recovered = circuit.retry_at.take().is_some();
        circuit.probe_in_flight = false;
        if recovered {
            tracing::info!("Stage Profiler v2 circuit recovered");
        }
    }

    fn release_probe(&self) {
        self.v2_circuit
            .lock()
            .expect("v2 circuit mutex poisoned")
            .probe_in_flight = false;
    }

    fn defer_failed_probe(&self) {
        let mut circuit = self.v2_circuit.lock().expect("v2 circuit mutex poisoned");
        if circuit.probe_in_flight && circuit.retry_at.is_some() {
            circuit.retry_at = Some(Instant::now() + self.rpc.stage_profiling_v2_probe_interval);
        }
        circuit.probe_in_flight = false;
    }

    pub fn release_v2_probe(&self) {
        self.release_probe();
    }

    fn url(&self) -> &str {
        self.rpc
            .url
            .strip_suffix("/")
            .unwrap_or(self.rpc.url.as_str())
    }

    /// Waits for a rate-limiter permit, retrying up to `num_of_retries` times,
    /// then executes the request once. The two failure modes stay distinct so
    /// each caller can map them onto its own error type - in particular a local
    /// rate-limiter exhaustion must never be mistaken for an upstream failure.
    async fn execute_rate_limited(&self, request: Request) -> Result<Response, RequestFailure> {
        for attempt in 1..=self.rpc.num_of_retries {
            let permit = timeout(
                Duration::from_millis(self.rpc.retry_delay_ms.into()),
                self.limiter.until_ready(),
            )
            .await;

            match permit {
                Ok(_) => {
                    return self
                        .http
                        .execute(request)
                        .await
                        .map_err(RequestFailure::Http)
                }
                Err(_) => tracing::info!(
                    attempt,
                    max_retries = self.rpc.num_of_retries,
                    "Rate limiter wait timed out, retrying"
                ),
            }
        }

        Err(RequestFailure::RateLimiterExhausted)
    }

    async fn make_request(&self, request: Request) -> anyhow::Result<Response> {
        self.execute_rate_limited(request)
            .await
            .map_err(|failure| match failure {
                RequestFailure::Http(error) => anyhow::anyhow!("HTTP request error: {}", error),
                RequestFailure::RateLimiterExhausted => anyhow::anyhow!(
                    "Exceeded maximum retry attempts ({}) waiting for rate limiter",
                    self.rpc.num_of_retries,
                ),
            })
    }

    async fn make_profiling_request(&self, request: Request) -> Result<Response, ProfilingError> {
        self.execute_rate_limited(request)
            .await
            .map_err(|failure| match failure {
                RequestFailure::Http(error) => ProfilingError::Transport(error),
                RequestFailure::RateLimiterExhausted => ProfilingError::LocalRateLimiter,
            })
    }
}

/// Outcome of [`Client::execute_rate_limited`] that the caller must translate.
enum RequestFailure {
    /// The request was executed and the HTTP client failed.
    Http(reqwest::Error),
    /// No permit was granted within the configured attempts; the request was
    /// never sent, so this says nothing about upstream availability.
    RateLimiterExhausted,
}

#[derive(serde::Deserialize)]
struct ApiResponse<T> {
    response: HashMap<String, T>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{
        matchers::{body_json, method, path},
        Mock, MockServer, ResponseTemplate,
    };

    #[test]
    fn fallback_http_classification_is_explicit() {
        assert!(ProfilingError::Http(reqwest::StatusCode::NOT_FOUND).is_v2_fallback_eligible());
        assert!(
            ProfilingError::Http(reqwest::StatusCode::SERVICE_UNAVAILABLE)
                .is_v2_fallback_eligible()
        );
        assert!(!ProfilingError::Http(reqwest::StatusCode::UNAUTHORIZED).is_v2_fallback_eligible());
        assert!(
            !ProfilingError::Http(reqwest::StatusCode::TOO_MANY_REQUESTS).is_v2_fallback_eligible()
        );
        assert!(!ProfilingError::LocalRateLimiter.is_v2_fallback_eligible());
    }

    #[test]
    fn only_one_probe_is_reserved() {
        let settings = RpcSettings {
            stage_profiling_v2_probe_interval: Duration::ZERO,
            ..Default::default()
        };
        let client = Client::new(settings);
        client.open_v2_circuit(&ProfilingError::EmptyResponse);
        assert!(client.v2_available_for_direct_request());
        assert!(!client.v2_available_for_direct_request());
    }

    #[test]
    fn concurrent_callers_reserve_exactly_one_probe() {
        let client = Client::new(RpcSettings {
            stage_profiling_v2_probe_interval: Duration::ZERO,
            ..Default::default()
        });
        client.open_v2_circuit(&ProfilingError::EmptyResponse);

        const CALLERS: usize = 16;
        let barrier = Arc::new(std::sync::Barrier::new(CALLERS));
        let handles: Vec<_> = (0..CALLERS)
            .map(|_| {
                let client = client.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    client.v2_available_for_direct_request()
                })
            })
            .collect();

        let reserved = handles
            .into_iter()
            .map(|handle| handle.join().expect("probe caller panicked"))
            .filter(|reserved| *reserved)
            .count();
        assert_eq!(reserved, 1);
    }

    #[tokio::test]
    async fn prefer_v2_falls_back_for_eligible_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/stage-profiling"))
            .and(body_json(serde_json::json!({"operationIds": ["op"]})))
            .respond_with(ResponseTemplate::new(503))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/stage-profiling"))
            .and(body_json(serde_json::json!({"operationIds": ["op"]})))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {"op": {"operationType": "PENDING", "metaInfo": null}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let client = Client::new(RpcSettings {
            url: server.uri(),
            ..Default::default()
        });
        assert!(matches!(
            client.get_operations_stages(vec!["op"]).await.unwrap(),
            ProfilingResponse::V1(_)
        ));
    }

    #[tokio::test]
    async fn strict_modes_do_not_cross_call_endpoints() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/stage-profiling"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": {"op": {"operationType": "PENDING", "metaInfo": null}}
            })))
            .expect(1)
            .mount(&server)
            .await;
        let v1_client = Client::new(RpcSettings {
            url: server.uri(),
            stage_profiling_mode: StageProfilingMode::V1Only,
            ..Default::default()
        });
        assert!(matches!(
            v1_client.get_operations_stages(vec!["op"]).await.unwrap(),
            ProfilingResponse::V1(_)
        ));

        let v2_client = Client::new(RpcSettings {
            url: server.uri(),
            stage_profiling_mode: StageProfilingMode::V2Only,
            ..Default::default()
        });
        assert!(v2_client.get_operations_stages(vec!["op"]).await.is_err());
    }

    #[tokio::test]
    async fn prefer_v2_does_not_mask_authentication_errors() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/stage-profiling"))
            .respond_with(ResponseTemplate::new(401))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/stage-profiling"))
            .respond_with(ResponseTemplate::new(200))
            .expect(0)
            .mount(&server)
            .await;
        let client = Client::new(RpcSettings {
            url: server.uri(),
            ..Default::default()
        });
        assert!(matches!(
            client.get_operations_stages(vec!["op"]).await,
            Err(ProfilingError::Http(reqwest::StatusCode::UNAUTHORIZED))
        ));
    }
}
