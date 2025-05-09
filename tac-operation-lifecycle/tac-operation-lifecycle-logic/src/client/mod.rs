use std::{collections::HashMap, num::NonZeroU32, sync::Arc};

use anyhow::Error;
use governor::{clock::DefaultClock, state::InMemoryState, Quota, RateLimiter};
use reqwest::{Client as HttpClient, Method, Request, Response};

pub mod models;
pub mod settings;

use settings::RpcSettings;
use tracing::Instrument;

use models::{
    operations::{OperationIdsApiResponse, Operations},
    profiling::{OperationData, StageProfilingApiResponse},
};
use tokio::time::{timeout, Duration};

type Limiter = RateLimiter<NotKeyed, InMemoryState, DefaultClock>;

use governor::state::NotKeyed;

#[derive(Clone)]
pub struct Client {
    rpc: RpcSettings,
    http: HttpClient,
    limiter: Arc<Limiter>,
}

impl Client {
    pub fn new(settings: RpcSettings) -> Self {
        let http = HttpClient::new();

        let quota = Quota::per_second(NonZeroU32::new(settings.request_per_second).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));

        Self {
            rpc: settings,
            http,
            limiter,
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
                tracing::error!(%url, status =? status, "Bad response");
                return Err(anyhow::anyhow!(
                    "HTTP error {}: {}",
                    status,
                    status.as_str()
                ));
            }

            let text = response.text().await?;

            if text.is_empty() {
                tracing::error!(%url, "Received empty response");
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

            if all_operations.len() >= parsed.total.try_into().unwrap() || count == 0 {
                break;
            }

            offset = all_operations.len();
        }

        Ok(all_operations)
    }

    pub async fn get_operations_stages(
        &self,
        id: Vec<&str>,
    ) -> Result<HashMap<String, OperationData>, Error> {
        let request_body = serde_json::json!({ "operationIds": id });
        let url = format!("{}/stage-profiling", self.url());
        let mut request = Request::new(Method::POST, url.parse()?);

        request
            .headers_mut()
            .insert("accept", "application/json".parse()?);
        request
            .headers_mut()
            .insert("Content-Type", "application/json".parse()?);
        request
            .body_mut()
            .replace(serde_json::to_vec(&request_body)?.into());

        let response = self
            .make_request(request)
            .instrument(tracing::debug_span!("get_operations_stages", url = url))
            .await?;

        if response.status().is_success() {
            let text = response.text().await?;
            if text.is_empty() {
                tracing::error!(url, "Received empty response");
                return Err(anyhow::anyhow!("Received empty response from {url}"));
            }

            match serde_json::from_str::<StageProfilingApiResponse>(&text) {
                Ok(response) => Ok(response.response),
                Err(e) => {
                    tracing::error!(url, err =? e, "Failed to parse staging response");
                    Err(e.into())
                }
            }
        } else {
            Err(anyhow::anyhow!(
                "HTTP error {}: {}",
                response.status().as_u16(),
                response.status().as_str()
            ))
        }
    }

    fn url(&self) -> &str {
        self.rpc
            .url
            .strip_suffix("/")
            .unwrap_or(self.rpc.url.as_str())
    }

    async fn make_request(&self, request: Request) -> anyhow::Result<Response> {
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
                        .map_err(|e| anyhow::anyhow!("HTTP request error: {}", e));
                }
                Err(_) => {
                    tracing::warn!(
                        attempt,
                        MAX_RETRIES =? self.rpc.num_of_retries,
                        "Rate limiter wait timed out, retrying..."
                    );
                }
            }
        }

        Err(anyhow::anyhow!(
            "Exceeded maximum retry attempts ({}) waiting for rate limiter",
            self.rpc.num_of_retries,
        ))
    }
}
