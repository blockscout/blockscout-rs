use api_client_framework::{
    Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig, serialize_query,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod decode_calldata {
    use super::*;

    pub struct DecodeCalldata {
        pub params: DecodeCalldataParams,
    }

    #[derive(Serialize, Clone, Debug, Default, PartialEq)]
    pub struct DecodeCalldataParams {
        pub calldata: String,
        pub address_hash: String,
    }

    impl Endpoint for DecodeCalldata {
        type Response = DecodedCalldata;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v2/utils/decode-calldata".to_string()
        }

        fn query(&self) -> Option<String> {
            serialize_query(&self.params)
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct DecodedCalldata {
        pub result: serde_json::Value,
    }
}

pub mod stats {
    use super::*;

    pub struct Stats {}

    impl Endpoint for Stats {
        type Response = StatsResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v2/stats".to_string()
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct StatsResponse {
        pub coin_price: Option<String>,
        pub coin_image: Option<String>,
        pub market_cap: Option<String>,
    }
}

pub mod node_api_config {
    use super::*;

    pub struct NodeApiConfig {}

    impl Endpoint for NodeApiConfig {
        type Response = NodeApiConfigResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/node-api/config".to_string()
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct NodeApiConfigResponse {
        pub envs: Envs,
    }

    #[derive(Debug, Deserialize, Default)]
    #[serde(rename_all = "SCREAMING_SNAKE_CASE", default)]
    pub struct Envs {
        pub next_public_marketplace_enabled: Option<String>,
        pub next_public_network_currency_name: Option<String>,
        pub next_public_network_currency_symbol: Option<String>,
        pub next_public_network_currency_decimals: Option<String>,
    }
}
