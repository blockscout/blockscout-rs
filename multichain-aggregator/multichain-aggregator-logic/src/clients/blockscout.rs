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
