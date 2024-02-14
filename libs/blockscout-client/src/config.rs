use reqwest_middleware::Middleware;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{str::FromStr, sync::Arc};

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
pub struct Config {
    chain_id: String,
    url: String,
    /// The value set in API_SENSITIVE_ENDPOINTS_KEY for the corresponding blockscout instance
    api_sensitive_endpoints_key: Option<String>,
    middleware_stack: Vec<Arc<dyn Middleware>>,
}

impl Config {
    pub fn new(chain_id: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
            url: url.into(),
            api_sensitive_endpoints_key: None,
            middleware_stack: vec![],
        }
    }

    pub fn with_api_sensitive_endpoints_key(mut self, key: String) -> Self {
        self.api_sensitive_endpoints_key = Some(key);
        self
    }

    pub fn with_retry_middleware(self, max_retries: u32) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retries);
        let middleware = RetryTransientMiddleware::new_with_policy(retry_policy);
        self.with_middleware(middleware)
    }

    pub fn with_middleware<M: Middleware>(self, middleware: M) -> Self {
        self.with_arc_middleware(Arc::new(middleware))
    }

    pub fn with_arc_middleware<M: Middleware>(mut self, middleware: Arc<M>) -> Self {
        self.middleware_stack.push(middleware);
        self
    }
}

#[derive(Clone)]
pub(super) struct ValidatedConfig {
    pub chain_id: String,
    pub url: url::Url,
    pub api_sensitive_endpoints_key: Option<String>,
    pub middleware_stack: Vec<Arc<dyn Middleware>>,
}

impl TryFrom<Config> for ValidatedConfig {
    type Error = BoxError;

    fn try_from(value: Config) -> Result<Self, Self::Error> {
        let url = url::Url::from_str(&value.url)?;

        Ok(Self {
            chain_id: value.chain_id,
            url,
            api_sensitive_endpoints_key: value.api_sensitive_endpoints_key,
            middleware_stack: value.middleware_stack,
        })
    }
}
