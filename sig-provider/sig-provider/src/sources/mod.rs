pub mod fourbyte;
pub mod sigeth;

use std::time::Duration;

use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use sig_provider_proto::blockscout::sig_provider::v1::{
    CreateSignaturesRequest, CreateSignaturesResponse, GetSignaturesRequest, GetSignaturesResponse,
};

#[async_trait::async_trait]
pub trait SignatureSource {
    async fn create_signatures(
        &self,
        request: CreateSignaturesRequest,
    ) -> Result<CreateSignaturesResponse, anyhow::Error>;
    async fn get_function_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error>;
    async fn get_event_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error>;

    // for errors
    fn host(&self) -> String;
}

pub fn new_client() -> ClientWithMiddleware {
    let retry_policy = ExponentialBackoff::builder().build_with_max_retries(3);
    ClientBuilder::new(
        reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
    )
    .with(RetryTransientMiddleware::new_with_policy(retry_policy))
    .build()
}
