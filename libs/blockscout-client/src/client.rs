use super::{config, Error, Result};
use reqwest_middleware::ClientWithMiddleware;
use url::Url;

#[derive(Clone, Debug)]
pub struct Client {
    chain_id: String,
    base_url: Url,
    api_sensitive_endpoints_key: Option<String>,
    request_client: ClientWithMiddleware,
}

impl Client {
    pub fn new(config: config::Config) -> Self {
        let config = match config::ValidatedConfig::try_from(config) {
            Ok(config) => config,
            Err(err) => panic!("Invalid blockscout client configuration: {err}"),
        };

        let request_client = {
            let client = reqwest::Client::new();
            let mut client_with_middleware = reqwest_middleware::ClientBuilder::new(client);
            for middleware in config.middleware_stack {
                client_with_middleware = client_with_middleware.with_arc(middleware);
            }
            client_with_middleware.build()
        };

        Self {
            chain_id: config.chain_id,
            base_url: config.url,
            api_sensitive_endpoints_key: config.api_sensitive_endpoints_key,
            request_client,
        }
    }

    pub fn chain_id(&self) -> &str {
        self.chain_id.as_ref()
    }

    pub fn api_sensitive_endpoints_key(&self) -> Option<&str> {
        self.api_sensitive_endpoints_key.as_deref()
    }

    pub fn build_url(&self, path: &str) -> Url {
        self.build_url_with_query(path, None)
    }

    pub(crate) fn build_url_with_query(&self, path: &str, query: Option<&str>) -> Url {
        let mut url = self.base_url.clone();
        url.set_path(path);
        url.set_query(query);
        url
    }

    pub(crate) async fn _post_request<U, Request, Response>(
        &self,
        url: U,
        request: &Request,
    ) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Request: serde::Serialize + ?Sized,
        Response: serde::de::DeserializeOwned,
    {
        let response = self.request_client.post(url).json(&request).send().await;
        Self::process_response(response).await
    }

    pub(crate) async fn get_request<U, Response>(&self, url: U) -> Result<Response>
    where
        U: reqwest::IntoUrl,
        Response: serde::de::DeserializeOwned,
    {
        self.get_request_with_headers(url, []).await
    }

    pub(crate) async fn get_request_with_headers<U, Response>(
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

    async fn process_response<Response>(
        response: reqwest_middleware::Result<reqwest::Response>,
    ) -> Result<Response>
    where
        Response: serde::de::DeserializeOwned,
    {
        match response {
            Ok(response) if response.status().is_success() => {
                let full = response.bytes().await?;

                let jd = &mut serde_json::Deserializer::from_slice(full.as_ref());
                serde_path_to_error::deserialize(jd).map_err(|err| Error::Decode(Box::new(err)))
            }
            Ok(response) => Err(Error::StatusCode(response)),
            Err(err) => Err(err.into()),
        }
    }
}
