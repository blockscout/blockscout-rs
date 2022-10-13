use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse, Success};
use crate::middleware::Middleware;
use reqwest::Url;
use reqwest_middleware::ClientWithMiddleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{num::NonZeroU32, sync::Arc, time::Duration};

pub struct SourcifyApiClientBuilder {
    host: Url,
    request_timeout: u64,
    verification_attempts: NonZeroU32,
    middleware_stack: Vec<Arc<dyn Middleware<Success>>>,
}

impl SourcifyApiClientBuilder {
    pub fn new(host: Url, request_timeout: u64, verification_attempts: NonZeroU32) -> Self {
        Self {
            host,
            request_timeout,
            verification_attempts,
            middleware_stack: vec![],
        }
    }

    /// Convenience method to attach middleware.
    ///
    /// If you need to keep a reference to the middleware after attaching, use [`with_arc`].
    ///
    /// [`with_arc`]: Self::with_arc
    pub fn with<M>(self, middleware: M) -> Self
    where
        M: Middleware<Success>,
    {
        self.with_arc(Arc::new(middleware))
    }

    /// Add middleware to the chain. [`with`] is more ergonomic if you don't need the `Arc`.
    ///
    /// [`with`]: Self::with
    pub fn with_arc(mut self, middleware: Arc<dyn Middleware<Success>>) -> Self {
        self.middleware_stack.push(middleware);
        self
    }

    /// Returns a `SourcifyApiClient` using this builder configuration.
    pub fn build(self) -> Result<SourcifyApiClient, reqwest::Error> {
        SourcifyApiClient::new(
            self.host,
            self.request_timeout,
            self.verification_attempts,
            self.middleware_stack,
        )
    }
}

pub struct SourcifyApiClient {
    host: Url,
    reqwest_client: ClientWithMiddleware,
    middleware_stack: Box<[Arc<dyn Middleware<Success>>]>,
}

impl SourcifyApiClient {
    /// See [`ClientBuilder`] for a more ergonomic way to build `SourcifyApiClient` instances.
    pub fn new<T>(
        host: Url,
        request_timeout: u64,
        verification_attempts: NonZeroU32,
        middleware_stack: T,
    ) -> Result<Self, reqwest::Error>
    where
        T: Into<Box<[Arc<dyn Middleware<Success>>]>>,
    {
        let retry_policy =
            ExponentialBackoff::builder().build_with_max_retries(verification_attempts.get());
        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(request_timeout))
            .build()?;
        let reqwest_client = reqwest_middleware::ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            host,
            reqwest_client,
            middleware_stack: middleware_stack.into(),
        })
    }

    pub(super) async fn verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, anyhow::Error> {
        self.reqwest_client
            .post(self.host.as_str())
            .json(&params)
            .send()
            .await?
            .json()
            .await
            .map_err(anyhow::Error::msg)
    }

    pub(super) async fn source_files_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, anyhow::Error> {
        let url = self
            .host
            .join(format!("files/any/{}/{}", &params.chain, &params.address).as_str())
            .expect("should be valid url");
        self.reqwest_client
            .get(url)
            .send()
            .await?
            .json()
            .await
            .map_err(anyhow::Error::msg)
    }

    pub fn middlewares(&self) -> &[Arc<dyn Middleware<Success>>] {
        self.middleware_stack.as_ref()
    }
}
