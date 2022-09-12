use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse};
use crate::network_helpers;
use std::num::NonZeroUsize;
use url::Url;

pub struct SourcifyApiClient {
    host: Url,
    request_timeout: u64,
    verification_attempts: NonZeroUsize,
}

impl SourcifyApiClient {
    pub fn new(host: Url, request_timeout: u64, verification_attempts: NonZeroUsize) -> Self {
        Self {
            host,
            request_timeout,
            verification_attempts,
        }
    }

    pub(super) async fn verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error> {
        network_helpers::make_retrying_request(self.verification_attempts, None, || async {
            let resp = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(self.request_timeout))
                .build()?
                .post(self.host.as_str())
                .json(&params)
                .send()
                .await?;
            resp.json().await
        })
        .await
    }

    pub(super) async fn source_files_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, reqwest::Error> {
        network_helpers::make_retrying_request(self.verification_attempts, None, || async {
            let url = self
                .host
                .join(format!("files/any/{}/{}", &params.chain, &params.address).as_str())
                .expect("should be valid url");
            let resp = reqwest::get(url).await?;

            resp.json().await
        })
        .await
    }
}
