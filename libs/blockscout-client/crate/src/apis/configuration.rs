/*
 * BlockScout API
 *
 * API for BlockScout web app
 *
 * The version of the OpenAPI document: 1.0.0
 * Contact: you@your-company.com
 * Generated by: https://openapi-generator.tech
 */
use reqwest_middleware::ClientBuilder;
use reqwest_retry::{policies::ExponentialBackoff, RetryTransientMiddleware};
use url::Url;

#[derive(Debug, Clone)]
pub struct Configuration {
    pub base_path: String,
    pub user_agent: Option<String>,
    pub client: reqwest_middleware::ClientWithMiddleware,
    pub basic_auth: Option<BasicAuth>,
    pub oauth_access_token: Option<String>,
    pub bearer_access_token: Option<String>,
    pub api_key: Option<ApiKey>,
    // TODO: take an oauth2 token source, similar to the go one
}

pub type BasicAuth = (String, Option<String>);

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub prefix: Option<String>,
    pub key: String,
}

impl Configuration {
    pub fn new(base_path: Url) -> Configuration {
        Configuration::default().with_base_path(base_path)
    }

    pub fn with_base_path(mut self, base_path: Url) -> Configuration {
        base_path
            .as_str()
            .trim_end_matches('/')
            .clone_into(&mut self.base_path);
        self
    }

    pub fn with_client(
        mut self,
        client: reqwest_middleware::ClientWithMiddleware,
    ) -> Configuration {
        self.client = client;
        self
    }

    pub fn with_client_max_retry(self, max_retry: u32) -> Configuration {
        let retry_policy = ExponentialBackoff::builder().build_with_max_retries(max_retry);
        self.with_client(
            ClientBuilder::new(reqwest::Client::new())
                .with(RetryTransientMiddleware::new_with_policy(retry_policy))
                .build(),
        )
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            base_path: "https://eth.blockscout.com".to_owned(),
            user_agent: Some("OpenAPI-Generator/1.0.0/rust".to_owned()),
            client: reqwest_middleware::ClientBuilder::new(reqwest::Client::new()).build(),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: None,
            api_key: None,
        }
    }
}
