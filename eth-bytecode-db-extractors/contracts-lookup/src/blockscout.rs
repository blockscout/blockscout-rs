use anyhow::Context;
use blockscout_display_bytes::Bytes;
use governor::{Quota, RateLimiter};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_rate_limiter::RateLimiterMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::{de::DeserializeOwned, Deserialize};
use std::{num::NonZeroU32, str::FromStr};
use url::Url;

#[derive(Clone, Debug, Deserialize)]
pub struct SearchContractResponse {
    pub message: Option<String>,
}

#[derive(Clone)]
pub struct Client {
    base_url: Url,
    request_client: ClientWithMiddleware,
    api_key: String,
}

impl Client {
    pub fn try_new(
        base_url: String,
        limit_requests_per_second: u32,
        api_key: String,
    ) -> anyhow::Result<Self> {
        let base_url = Url::from_str(&base_url).context("invalid blockscout base url")?;
        let max_burst = NonZeroU32::new(limit_requests_per_second)
            .ok_or_else(|| anyhow::anyhow!("invalid limit requests per second"))?;

        let rate_limiter = RateLimiter::direct(Quota::per_second(max_burst));

        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .with(RateLimiterMiddleware::new(rate_limiter))
            .build();

        Ok(Self {
            base_url,
            request_client: client,
            api_key,
        })
    }

    pub async fn search_contract(
        &self,
        contract_address: Bytes,
    ) -> anyhow::Result<SearchContractResponse> {
        let url = {
            let path = format!("/api/v2/import/smart-contracts/{contract_address}");
            let mut url = self.base_url.clone();
            url.set_path(&path);
            url
        };

        self.send_request(url, [("x-api-key", &self.api_key)])
            .await
            .context("sending request")
    }

    async fn send_request<Response: DeserializeOwned>(
        &self,
        url: Url,
        headers: impl IntoIterator<Item = (impl AsRef<str>, impl AsRef<str>)>,
    ) -> anyhow::Result<Response> {
        let mut request = self.request_client.get(url);
        for (header_key, header_value) in headers {
            request = request.header(header_key.as_ref(), header_value.as_ref());
        }

        let response = request.send().await.context("sending request failed")?;

        // Continue only in case if request results is success
        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Invalid status code get as a result: {}",
                response.status()
            ));
        }

        let result = response
            .text()
            .await
            .context("deserializing response into string failed")?;
        let jd = &mut serde_json::Deserializer::from_str(&result);
        serde_path_to_error::deserialize(jd).context("deserializing response failed")
    }
}
