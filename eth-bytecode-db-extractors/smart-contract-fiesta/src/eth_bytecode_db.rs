pub use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2::{
    solidity_verifier_client::SolidityVerifierClient, verify_response,
    vyper_verifier_client::VyperVerifierClient, BytecodeType, SearchSourcesRequest,
    SearchSourcesResponse, Source, VerificationMetadata, VerifyResponse,
    VerifySolidityMultiPartRequest, VerifySolidityStandardJsonRequest, VerifyVyperMultiPartRequest,
};

use anyhow::Context;
use serde::{de::DeserializeOwned, Serialize};
use std::{str::FromStr, time::Duration};
use url::Url;

#[derive(Clone)]
pub struct Client {
    url: Url,
    request_client: reqwest::Client,
}

impl Client {
    pub fn try_new(service_url: String) -> anyhow::Result<Self> {
        let service_url =
            Url::from_str(&service_url).context("invalid eth_bytecode_db service url")?;

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .build()
            .unwrap();

        Ok(Self {
            url: service_url,
            request_client: client,
        })
    }

    pub async fn verify_solidity_multi_part(
        &self,
        request: VerifySolidityMultiPartRequest,
    ) -> anyhow::Result<VerifyResponse> {
        let path = "/api/v2/verifier/solidity/sources:verify-multi-part";
        self.send_request(path, request).await
    }

    pub async fn verify_solidity_standard_json(
        &self,
        request: VerifySolidityStandardJsonRequest,
    ) -> anyhow::Result<VerifyResponse> {
        let path = "/api/v2/verifier/solidity/sources:verify-standard-json";
        self.send_request(path, request).await
    }

    pub async fn verify_vyper_multi_part(
        &self,
        request: VerifyVyperMultiPartRequest,
    ) -> anyhow::Result<VerifyResponse> {
        let path = "/api/v2/verifier/vyper/sources:verify-multi-part";
        self.send_request(path, request).await
    }

    pub async fn search_sources(
        &self,
        request: SearchSourcesRequest,
    ) -> anyhow::Result<SearchSourcesResponse> {
        let path = "/api/v2/bytecodes/sources:search";
        self.send_request(path, request).await
    }

    async fn send_request<Request: Serialize, Response: DeserializeOwned>(
        &self,
        path: &str,
        request: Request,
    ) -> anyhow::Result<Response> {
        let url = {
            let mut url = self.url.clone();
            url.set_path(path);
            url
        };
        let response = self
            .request_client
            .post(url)
            .json(&request)
            .send()
            .await
            .context("error sending request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "verification http request failed with the following status: {}, message: {:?}",
                response.status(),
                response.text().await
            ));
        }

        response
            .json::<Response>()
            .await
            .context("verify response deserialization failed")
    }
}
