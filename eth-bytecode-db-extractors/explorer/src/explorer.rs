use crate::Settings;
use anyhow::Context;
use api_client_framework::{HttpApiClient, HttpApiClientConfig};
use std::num::NonZero;
use std::sync::Arc;

#[derive(Clone)]
pub struct Explorer {
    pub client: HttpApiClient,
    pub chain_id: String,
    pub api_key: String,
}

impl Explorer {
    pub fn new(settings: &Settings) -> Result<Self, anyhow::Error> {
        let mut config = HttpApiClientConfig::default();

        let limit_requests_per_second =
            NonZero::try_from(settings.limit_requests_per_second).unwrap();
        let rate_limiter_middleware =
            reqwest_rate_limiter::DefaultRateLimiterMiddleware::per_second(
                limit_requests_per_second,
            );
        config.middlewares.push(Arc::new(rate_limiter_middleware));

        let client = HttpApiClient::new(settings.explorer_url.clone(), config)
            .context("explorer initialization failed")?;

        Ok(Self {
            client,
            chain_id: settings.chain_id.clone(),
            api_key: settings.explorer_api_key.clone(),
        })
    }
}

pub mod get_source_code {
    use api_client_framework::reqwest::Method;
    use serde::{Serialize, Deserialize};
    use blockscout_display_bytes::ToHex;

    #[derive(Clone, Debug)]
    pub struct GetSourceCode {
        pub address: Vec<u8>,
        pub chain_id: String,
        pub api_key: String,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct GetSourceCodeResponse {
        pub result: Vec<serde_json::Value>,
    }

    impl api_client_framework::Endpoint for GetSourceCode {
        type Response = GetSourceCodeResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/v2/api".to_string()
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            struct Query {
                chainid: String,
                module: String,
                action: String,
                address: String,
                apikey: String,
            }

            let query = Query {
                chainid: self.chain_id.clone(),
                module: "contract".to_string(),
                action: "getsourcecode".to_string(),
                address: self.address.to_hex(),
                apikey: self.api_key.clone(),
            };
            api_client_framework::serialize_query(&query)
        }
    }
}