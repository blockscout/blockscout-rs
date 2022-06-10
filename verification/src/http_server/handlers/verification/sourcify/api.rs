use crate::{VerificationResponse, VerificationResult};
use actix_web::{error, error::Error};
use reqwest::Url;
use std::sync::Arc;

use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse, Files};

#[async_trait::async_trait]
pub(super) trait SourcifyApi {
    async fn verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error>;

    async fn source_files_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, reqwest::Error>;
}

pub struct SourcifyApiClient {
    host: Url,
    timeout: u64,
    verification_attempts: u64,
}

impl SourcifyApiClient {
    pub fn new(host: Url) -> Self {
        Self {
            host,
            timeout: 10,
            verification_attempts: 3,
        }
    }

    async fn _verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error> {
        let resp = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.timeout))
            .build()?
            .post(self.host.as_str())
            .json(&params)
            .send()
            .await?;

        resp.json().await
    }
}

#[async_trait::async_trait]
impl SourcifyApi for SourcifyApiClient {
    async fn verification_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error> {
        let mut resp = self._verification_request(params).await;
        for _ in 1..self.verification_attempts {
            if resp.is_ok() {
                return resp;
            }
            resp = self._verification_request(params).await;
        }
        resp
    }

    async fn source_files_request(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, reqwest::Error> {
        let url = self
            .host
            .join(format!("files/any/{}/{}", &params.chain, &params.address).as_str())
            .expect("should be valid url");
        let resp = reqwest::get(url).await?;

        resp.json().await
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
