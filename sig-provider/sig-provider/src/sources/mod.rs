pub mod fourbyte;
pub mod sigeth;

use async_trait::async_trait;
use mockall::automock;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::time::Duration;

#[automock]
#[async_trait]
pub trait SignatureSource {
    async fn create_signatures(&self, abi: &str) -> Result<(), anyhow::Error>;
    // Resulting signatures should be sorted in priority descending order (first - max priority)
    async fn get_function_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error>;
    // Resulting signatures should be sorted in priority descending order (first - max priority)
    async fn get_event_signatures(&self, hex: &str) -> Result<Vec<String>, anyhow::Error>;

    // for errors
    fn source(&self) -> String;
}

pub fn new_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(1);
    ClientBuilder::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
    )
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build()
}
