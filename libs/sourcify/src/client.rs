// SPDX-License-Identifier: LicenseRef-Blockscout

use reqwest_middleware::{ClientWithMiddleware, Middleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::{str::FromStr, sync::Arc, time::Duration};
use url::Url;

/// Default interval between polls of an asynchronous verification job.
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
/// Default maximum number of times an asynchronous verification job is polled
/// before giving up. Combined with [`DEFAULT_POLL_INTERVAL`] this yields a
/// ~120 second ceiling, which comfortably covers Etherscan imports.
pub const DEFAULT_MAX_POLL_ATTEMPTS: u32 = 120;

mod retryable_strategy {
    use reqwest::StatusCode;
    use reqwest_middleware::Error;
    use reqwest_retry::{Retryable, RetryableStrategy};

    pub struct SourcifyRetryableStrategy;

    impl RetryableStrategy for SourcifyRetryableStrategy {
        fn handle(&self, res: &Result<reqwest::Response, Error>) -> Option<Retryable> {
            match res {
                Ok(success) => default_on_request_success(success),
                Err(error) => reqwest_retry::default_on_request_failure(error),
            }
        }
    }

    // The strategy differs from `reqwest_retry::default_on_request_success`
    // by considering 500 errors as Fatal instead of Transient.
    // The reason is that Sourcify uses 500 code to propagate fatal internal errors,
    // which will not be resolved on retry and which we would like to get early to process.
    fn default_on_request_success(success: &reqwest::Response) -> Option<Retryable> {
        let status = success.status();
        if status.is_server_error() && status != StatusCode::INTERNAL_SERVER_ERROR {
            Some(Retryable::Transient)
        } else if status.is_success() {
            None
        } else if status == StatusCode::REQUEST_TIMEOUT || status == StatusCode::TOO_MANY_REQUESTS {
            Some(Retryable::Transient)
        } else {
            Some(Retryable::Fatal)
        }
    }
}

#[derive(Clone)]
pub struct ClientBuilder {
    base_url: Url,
    max_retries: u32,
    request_timeout: Option<Duration>,
    poll_interval: Duration,
    max_poll_attempts: u32,
    middleware_stack: Vec<Arc<dyn Middleware>>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self {
            base_url: Url::from_str("https://sourcify.dev/server/").unwrap(),
            max_retries: 3,
            request_timeout: None,
            poll_interval: DEFAULT_POLL_INTERVAL,
            max_poll_attempts: DEFAULT_MAX_POLL_ATTEMPTS,
            middleware_stack: vec![],
        }
    }
}

impl ClientBuilder {
    pub fn try_base_url(mut self, base_url: &str) -> Result<Self, String> {
        let base_url = Url::from_str(base_url).map_err(|err| err.to_string())?;
        self.base_url = base_url;

        Ok(self)
    }

    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Per-request timeout applied to every HTTP call the client makes.
    pub fn request_timeout(mut self, request_timeout: Duration) -> Self {
        self.request_timeout = Some(request_timeout);
        self
    }

    /// Interval to wait between polls of an asynchronous (v2) verification job.
    pub fn poll_interval(mut self, poll_interval: Duration) -> Self {
        self.poll_interval = poll_interval;
        self
    }

    /// Maximum number of times an asynchronous (v2) verification job is polled
    /// before the client gives up with an internal error.
    pub fn max_poll_attempts(mut self, max_poll_attempts: u32) -> Self {
        self.max_poll_attempts = max_poll_attempts;
        self
    }

    pub fn with_middleware<M: Middleware>(self, middleware: M) -> Self {
        self.with_arc_middleware(Arc::new(middleware))
    }

    pub fn with_arc_middleware<M: Middleware>(mut self, middleware: Arc<M>) -> Self {
        self.middleware_stack.push(middleware);
        self
    }

    pub fn build(self) -> Client {
        let reqwest_client = match self.request_timeout {
            Some(timeout) => reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("failed to build reqwest client"),
            None => reqwest::Client::new(),
        };
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(self.max_retries);
        let mut client_builder = reqwest_middleware::ClientBuilder::new(reqwest_client).with(
            RetryTransientMiddleware::new_with_policy_and_strategy(
                retry_policy,
                retryable_strategy::SourcifyRetryableStrategy,
            ),
        );
        for middleware in self.middleware_stack {
            client_builder = client_builder.with_arc(middleware);
        }
        let client = client_builder.build();

        Client {
            base_url: self.base_url,
            reqwest_client: client,
            poll_interval: self.poll_interval,
            max_poll_attempts: self.max_poll_attempts,
        }
    }
}

#[derive(Clone)]
pub struct Client {
    pub(crate) base_url: Url,
    pub(crate) reqwest_client: ClientWithMiddleware,
    pub(crate) poll_interval: Duration,
    pub(crate) max_poll_attempts: u32,
}

impl Default for Client {
    /// Initializes [`Client`] with base url set to "https://sourcify.dev/server/",
    /// and total duration to 60 seconds.
    fn default() -> Self {
        ClientBuilder::default().build()
    }
}

impl Client {
    pub(crate) fn generate_url(&self, route: &str) -> Url {
        self.base_url.join(route).unwrap()
    }
}
