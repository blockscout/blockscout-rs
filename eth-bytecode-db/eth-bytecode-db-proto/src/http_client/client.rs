use super::{config, Error, Result};
use crate::blockscout::eth_bytecode_db::v2 as proto;

const API_KEY_NAME: &str = "x-api-key";

#[derive(Clone, Debug)]
pub struct Client {
    base_url: url::Url,
    api_key: Option<String>,
    request_client: reqwest_middleware::ClientWithMiddleware,
}

impl Client {
    pub async fn new(config: config::Config) -> Self {
        let config = match config::ValidatedConfig::try_from(config) {
            Ok(config) => config,
            Err(err) => panic!("Invalid client configuration: {err}"),
        };

        let request_client = {
            let client = reqwest::Client::new();
            let mut client_with_middleware = reqwest_middleware::ClientBuilder::new(client);
            for middleware in config.middleware_stack {
                client_with_middleware = client_with_middleware.with_arc(middleware);
            }
            client_with_middleware.build()
        };

        let client = Self {
            base_url: config.url,
            api_key: config.api_key,
            request_client,
        };

        if config.probe_url {
            if let Err(err) = health_client::health(&client, Default::default()).await {
                panic!("Cannot establish a connection with eth-bytecode-db client: {err}")
            }
        }

        client
    }

    fn build_url(&self, path: &str) -> url::Url {
        self.build_url_with_query(path, None)
    }

    fn build_url_with_query(&self, path: &str, query: Option<&str>) -> url::Url {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url.set_query(query);
        url
    }

    async fn post_request<U, Request, Response>(
        &self,
        url: U,
        request: &Request,
    ) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Request: serde::Serialize + ?Sized,
        Response: serde::de::DeserializeOwned,
    {
        self.post_request_with_headers(url, request, []).await
    }

    async fn post_request_with_headers<U, Request, Response>(
        &self,
        url: U,
        request: &Request,
        headers: impl IntoIterator<Item = (&str, &str)>,
    ) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Request: serde::Serialize + ?Sized,
        Response: serde::de::DeserializeOwned,
    {
        let mut request = self.request_client.post(url).json(&request);
        for (key, value) in headers {
            request = request.header(key, value);
        }
        let response = request.send().await;
        Self::process_response(response).await
    }

    async fn get_request<U, Response>(&self, url: U) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Response: serde::de::DeserializeOwned,
    {
        self.get_request_with_headers(url, []).await
    }

    async fn get_request_with_headers<U, Response>(
        &self,
        url: U,
        headers: impl IntoIterator<Item = (&str, &str)>,
    ) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Response: serde::de::DeserializeOwned,
    {
        let mut request = self.request_client.get(url);
        for (key, value) in headers {
            request = request.header(key, value);
        }
        let response = request.send().await;
        Self::process_response(response).await
    }

    fn key_headers(&self) -> Vec<(&str, &str)> {
        match self.api_key.as_deref() {
            None => vec![],
            Some(key) => vec![(API_KEY_NAME, key)],
        }
    }

    async fn process_response<Response>(
        response: reqwest_middleware::Result<reqwest::Response>,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
    {
        match response {
            Ok(response) if response.status().is_success() => Ok(response.json().await?),
            Ok(response) => Err(Error::StatusCode(response)),
            Err(err) => Err(err.into()),
        }
    }
}

pub mod database_client {
    use super::{proto, Client, Result};

    pub async fn search_sources(
        client: &Client,
        request: proto::SearchSourcesRequest,
    ) -> Result<proto::SearchSourcesResponse> {
        let path = "/api/v2/bytecodes/sources:search";
        client.post_request(client.build_url(path), &request).await
    }
    pub async fn search_sourcify_sources(
        client: &Client,
        request: proto::SearchSourcifySourcesRequest,
    ) -> Result<proto::SearchSourcesResponse> {
        let path = "/api/v2/bytecodes/sources:search-sourcify";
        client.post_request(client.build_url(path), &request).await
    }
    pub async fn search_alliance_sources(
        client: &Client,
        request: proto::SearchAllianceSourcesRequest,
    ) -> Result<proto::SearchSourcesResponse> {
        let path = "/api/v2/bytecodes/sources:search-alliance";
        client.post_request(client.build_url(path), &request).await
    }
    pub async fn search_all_sources(
        client: &Client,
        request: proto::SearchAllSourcesRequest,
    ) -> Result<proto::SearchAllSourcesResponse> {
        let path = "/api/v2/bytecodes/sources:search-all";
        client.post_request(client.build_url(path), &request).await
    }
    pub async fn search_event_descriptions(
        client: &Client,
        request: proto::SearchEventDescriptionsRequest,
    ) -> Result<proto::SearchEventDescriptionsResponse> {
        let path = "/api/v2/event-descriptions:search";
        client.post_request(client.build_url(path), &request).await
    }
    pub async fn batch_search_event_descriptions(
        client: &Client,
        request: proto::BatchSearchEventDescriptionsRequest,
    ) -> Result<proto::BatchSearchEventDescriptionsResponse> {
        let path = "/api/v2/event-descriptions:batch-search";
        client.post_request(client.build_url(path), &request).await
    }
}

pub mod solidity_verifier_client {
    use super::{proto, Client, Result};

    pub async fn verify_multi_part(
        client: &Client,
        request: proto::VerifySolidityMultiPartRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/solidity/sources:verify-multi-part";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn verify_standard_json(
        client: &Client,
        request: proto::VerifySolidityStandardJsonRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/solidity/sources:verify-standard-json";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn list_compiler_versions(
        client: &Client,
        _request: proto::ListCompilerVersionsRequest,
    ) -> Result<proto::ListCompilerVersionsResponse> {
        let path = "/api/v2/verifier/solidity/versions";
        client.get_request(client.build_url(path)).await
    }
}

pub mod vyper_verifier_client {
    use super::{proto, Client, Result};

    pub async fn verify_multi_part(
        client: &Client,
        request: proto::VerifyVyperMultiPartRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/vyper/sources:verify-multi-part";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn verify_standard_json(
        client: &Client,
        request: proto::VerifyVyperStandardJsonRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/vyper/sources:verify-standard-json";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn list_compiler_versions(
        client: &Client,
        _request: proto::ListCompilerVersionsRequest,
    ) -> Result<proto::ListCompilerVersionsResponse> {
        let path = "/api/v2/verifier/vyper/versions";
        client.get_request(client.build_url(path)).await
    }
}

pub mod sourcify_verifier_client {
    use super::{proto, Client, Result};

    pub async fn verify(
        client: &Client,
        request: proto::VerifySourcifyRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/sourcify/sources:verify";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn verify_from_etherscan(
        client: &Client,
        request: proto::VerifyFromEtherscanSourcifyRequest,
    ) -> Result<proto::VerifyResponse> {
        let path = "/api/v2/verifier/sourcify/sources:verify-from-etherscan";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }
}

pub mod verifier_alliance_client {
    use super::{proto, Client, Result};

    pub async fn batch_import_solidity_multi_part(
        client: &Client,
        request: proto::VerifierAllianceBatchImportSolidityMultiPartRequest,
    ) -> Result<proto::VerifierAllianceBatchImportResponse> {
        let path = "/api/v2/alliance/solidity/multi-part:batch-import";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }

    pub async fn batch_import_solidity_standard_json(
        client: &Client,
        request: proto::VerifierAllianceBatchImportSolidityStandardJsonRequest,
    ) -> Result<proto::VerifierAllianceBatchImportResponse> {
        let path = "/api/v2/alliance/solidity/standard-json:batch-import";
        client
            .post_request_with_headers(client.build_url(path), &request, client.key_headers())
            .await
    }
}

pub mod health_client {
    use super::{proto, Client, Result};

    pub async fn health(
        client: &Client,
        request: proto::HealthCheckRequest,
    ) -> Result<proto::HealthCheckResponse> {
        let path = "/health";
        let query = Some(format!("service={}", request.service.unwrap_or_default()));
        client
            .get_request(client.build_url_with_query(path, query.as_deref()))
            .await
    }
}
