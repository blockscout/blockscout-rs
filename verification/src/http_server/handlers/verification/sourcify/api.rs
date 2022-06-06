use super::types::{ApiFilesResponse, ApiRequest, ApiVerificationResponse};

pub(super) struct SoucifyApiClient {
    host: String,
}

impl SoucifyApiClient {
    pub(super) fn new(host: &str) -> Self {
        Self {
            host: host.to_string(),
        }
    }

    pub(super) async fn verification(
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

    pub(super) async fn source_files(
        &self,
        params: &ApiRequest,
    ) -> Result<ApiFilesResponse, reqwest::Error> {
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
