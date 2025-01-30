use api_client_framework::{
    serialize_query, Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig,
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod search_dapps {
    use super::*;

    pub struct SearchDapps {
        pub params: SearchDappsParams,
    }

    #[derive(Serialize, Clone, Debug, Default, PartialEq)]
    pub struct SearchDappsParams {
        pub query: String,
    }

    impl Endpoint for SearchDapps {
        type Response = Vec<DappWithChainId>;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/marketplace/dapps:search".to_string()
        }

        fn query(&self) -> Option<String> {
            serialize_query(&self.params)
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DappWithChainId {
        pub dapp: Dapp,
        pub chain_id: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Dapp {
        pub id: String,
        pub title: String,
        pub logo: String,
        pub short_description: String,
    }
}

pub mod list_categories {
    use super::*;

    pub struct ListCategories {}

    impl Endpoint for ListCategories {
        type Response = Vec<String>;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/marketplace/categories".to_string()
        }
    }
}

pub mod list_chains {
    use super::*;

    pub struct ListChains {}

    impl Endpoint for ListChains {
        type Response = Vec<String>;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/marketplace/chains".to_string()
        }
    }
}
