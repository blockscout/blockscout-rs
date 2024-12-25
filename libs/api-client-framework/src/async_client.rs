use super::endpoint::Endpoint;
use crate::Error;
use reqwest::{header::HeaderMap, Response, StatusCode};
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub struct HttpApiClientConfig {
    /// The maximum time limit for an API request. If a request takes longer than this, it will be
    /// cancelled.
    pub http_timeout: Duration,
    /// Maximum number of allowed retries attempts. Defaults to 1.
    pub max_retries: u32,
    /// A default set of HTTP headers which will be sent with each API request.
    pub default_headers: HeaderMap,
}

impl Default for HttpApiClientConfig {
    fn default() -> Self {
        Self {
            http_timeout: Duration::from_secs(30),
            max_retries: 1,
            default_headers: HeaderMap::default(),
        }
    }
}

#[derive(Clone)]
pub struct HttpApiClient {
    base_url: url::Url,
    http_client: reqwest_middleware::ClientWithMiddleware,
}

impl HttpApiClient {
    pub fn new(base_url: url::Url, config: HttpApiClientConfig) -> Result<Self, Error> {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(config.max_retries);
        let reqwest_client = reqwest::Client::builder()
            .default_headers(config.default_headers)
            .timeout(config.http_timeout)
            .build()?;
        let client = ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Ok(Self {
            base_url,
            http_client: client,
        })
    }

    /// Issue an API request of the given type.
    pub async fn request<EndpointType: Endpoint>(
        &self,
        endpoint: &EndpointType,
    ) -> Result<<EndpointType as Endpoint>::Response, Error> {
        // Build the request
        let mut request = self
            .http_client
            .request(endpoint.method(), endpoint.url(&self.base_url));

        if let Some(body) = endpoint.body() {
            request = request.body(body);
            request = request.header(
                reqwest::header::CONTENT_TYPE,
                endpoint.content_type().as_ref(),
            );
        }

        let response = request.send().await?;
        process_api_response(response).await
    }
}

async fn process_api_response<T: for<'a> Deserialize<'a>>(response: Response) -> Result<T, Error> {
    let status = response.status();
    match status {
        status if status.is_success() => (),
        StatusCode::NOT_FOUND => return Err(Error::NotFound),
        status => {
            return Err(Error::InvalidStatusCode {
                status_code: status,
                message: response.text().await?,
            })
        }
    }

    let raw_value = response.bytes().await?;
    let deserializer = &mut serde_json::Deserializer::from_slice(raw_value.as_ref());
    let value: T = serde_path_to_error::deserialize(deserializer)?;
    Ok(value)
}
