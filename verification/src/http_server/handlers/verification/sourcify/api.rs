use crate::{VerificationResponse, VerificationResult};
use actix_web::{error, error::Error};

use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse, Files};

#[async_trait::async_trait]
pub(super) trait SourcifyApi {
    async fn verification(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error>;

    async fn source_files(&self, params: &ApiRequest) -> Result<ApiFilesResponse, reqwest::Error>;
}

pub(super) struct SoucifyApiClient {
    host: String,
}

impl SoucifyApiClient {
    pub fn new(host: &str) -> Self {
        Self {
            host: host.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl SourcifyApi for SoucifyApiClient {
    async fn verification(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiVerificationResponse, reqwest::Error> {
        let resp = reqwest::Client::new()
            .post(&self.host)
            .json(&params)
            .send()
            .await?;

        resp.json().await
    }

    async fn source_files(&self, params: &ApiRequest) -> Result<ApiFilesResponse, reqwest::Error> {
        let url = format!(
            "{host}/files/any/{chain}/{address}",
            host = self.host,
            chain = params.chain,
            address = params.address,
        );
        let resp = reqwest::get(url).await?;

        resp.json().await
    }
}

pub(super) async fn verify_using_sourcify_client(
    sourcify_client: impl SourcifyApi,
    params: ApiRequest,
) -> Result<VerificationResponse, Error> {
    let response = sourcify_client
        .verification(&params)
        .await
        .map_err(error::ErrorInternalServerError)?;

    match response {
        ApiVerificationResponse::Verified { result: api_result } => {
            let files = {
                let contract_was_already_verified = api_result
                    .first()
                    .ok_or_else(|| error::ErrorInternalServerError("sourcify empty response"))?
                    .storage_timestamp
                    .is_some();
                if contract_was_already_verified {
                    let api_files_response = sourcify_client
                        .source_files(&params)
                        .await
                        .map_err(error::ErrorInternalServerError)?;
                    Files::try_from(api_files_response).map_err(error::ErrorInternalServerError)?
                } else {
                    params.files
                }
            };
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

#[cfg(test)]
mod tests {
    // TODO: add tests in this PR, using mocked SourcifyApi
}
