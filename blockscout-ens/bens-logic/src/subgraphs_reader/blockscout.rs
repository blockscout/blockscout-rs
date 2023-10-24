use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};

pub struct BlockscoutClient {
    host: url::Url,
    inner: ClientWithMiddleware,
}

impl BlockscoutClient {
    pub fn new(host: url::Url) -> Self {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
        let client = ClientBuilder::new(reqwest::Client::new())
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();
        Self {
            host,
            inner: client,
        }
    }
}


impl BlockscoutClient {
    // todo: write transaction api
    // pub async tx()
}