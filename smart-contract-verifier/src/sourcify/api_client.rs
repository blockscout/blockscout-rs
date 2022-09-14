use reqwest::Url;
use std::time::Duration;

use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::num::NonZeroU32;

pub struct SourcifyApiClient {
    host: Url,
    reqwest_client: ClientWithMiddleware,
}

impl SourcifyApiClient {
    pub fn new(
        host: Url,
        request_timeout: u64,
        verification_attempts: NonZeroU32,
    ) -> Result<Self, reqwest::Error> {
        let retry_policy =
            ExponentialBackoff::builder().build_with_max_retries(verification_attempts.get());
        let reqwest_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(request_timeout))
            .build()?;
        let reqwest_client = ClientBuilder::new(reqwest_client)
            .with(RetryTransientMiddleware::new_with_policy(retry_policy))
            .build();

        Ok(Self {
            host,
            reqwest_client,
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
}
