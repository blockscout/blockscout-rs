use crate::{VerificationResponse, VerificationResult};
use actix_web::{error, error::Error};
use reqwest::Url;
use std::{sync::Arc, time::Duration};

use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse, Files};
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use std::num::NonZeroU32;

#[async_trait::async_trait]
pub(super) trait SourcifyApi {
    async fn verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, anyhow::Error>;

    async fn source_files_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, anyhow::Error>;
}

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
}

#[async_trait::async_trait]
impl SourcifyApi for SourcifyApiClient {
    async fn verification_request(
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

    async fn source_files_request(
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

pub(super) async fn verify_using_sourcify_client(
    sourcify_client: Arc<impl SourcifyApi>,
    params: ApiRequest,
) -> Result<VerificationResponse, Error> {
    let response = sourcify_client
        .verification_request(&params)
        .await
        .map_err(error::ErrorInternalServerError)?;

    match response {
        ApiVerificationResponse::Verified { result: _ } => {
            let api_files_response = sourcify_client
                .source_files_request(&params)
                .await
                .map_err(error::ErrorInternalServerError)?;
            let files =
                Files::try_from(api_files_response).map_err(error::ErrorInternalServerError)?;
            let result = VerificationResult::try_from(files).map_err(error::ErrorBadRequest)?;
            Ok(VerificationResponse::ok(result))
        }
        ApiVerificationResponse::Error { error } => Ok(VerificationResponse::err(error)),
        ApiVerificationResponse::ValidationErrors { message, errors } => {
            let error_message = format!("{}: {:?}", message, errors);
            Err(error::ErrorBadRequest(error_message))
        }
    }
}
