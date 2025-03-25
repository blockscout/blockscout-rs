use crate::types::ChainId;
use api_client_framework::{
    serialize_query, Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_with::{formats::CommaSeparator, serde_as, StringWithSeparator};
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod list_chains {
    use super::*;

    pub struct ListChains {}

    impl Endpoint for ListChains {
        type Response = ListChainsResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/token-infos/chains".to_string()
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct ListChainsResponse {
        pub chains: Vec<u64>,
    }
}

pub mod search_token_infos {
    use super::*;

    pub struct SearchTokenInfos {
        pub params: SearchTokenInfosParams,
    }

    #[serde_as]
    #[serde_with::skip_serializing_none]
    #[derive(Serialize, Clone, Debug, Default, PartialEq)]
    pub struct SearchTokenInfosParams {
        pub query: String,
        #[serde_as(as = "StringWithSeparator::<CommaSeparator, ChainId>")]
        #[serde(skip_serializing_if = "Vec::is_empty")]
        pub chain_id: Vec<ChainId>,
        pub page_size: Option<u32>,
        pub page_token: Option<String>,
    }

    impl Endpoint for SearchTokenInfos {
        type Response = TokenInfoSearchResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/token-infos:search".to_string()
        }

        fn query(&self) -> Option<String> {
            serialize_query(&self.params)
        }
    }
    #[derive(Debug, Deserialize)]
    pub struct TokenInfoSearchResponse {
        pub token_infos: Vec<TokenInfo>,
        pub next_page_params: Option<Pagination>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct TokenInfo {
        pub token_address: String,
        pub chain_id: String,
        pub icon_url: String,
        pub token_name: Option<String>,
        pub token_symbol: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Pagination {
        pub page_token: String,
        pub page_size: u32,
    }
}
