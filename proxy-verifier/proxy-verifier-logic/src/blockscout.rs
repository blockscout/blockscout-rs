use api_client_framework::{reqwest::Method, Endpoint, Error, HttpApiClient};
use blockscout_display_bytes::ToHex;
use serde::Deserialize;

const API_KEY_HEADER: &str = "x-api-key";

#[derive(Clone)]
pub struct Client {
    http_client: HttpApiClient,
    base_url: url::Url,
    api_key: http::HeaderValue,
}

impl Client {
    pub async fn new(base_url: url::Url, api_key: String) -> Self {
        let api_key = http::HeaderValue::from_str(&api_key).unwrap_or_else(|err| {
            panic!("{base_url} - api key is not a valid header value; err={err}")
        });

        let http_client = HttpApiClient::new(base_url.clone(), Default::default())
            .unwrap_or_else(|err| panic!("{base_url} - cannot build an http client: {err:#?}"));

        Self {
            http_client,
            base_url,
            api_key,
        }
    }

    pub async fn request<EndpointType: Endpoint>(
        &self,
        endpoint: &EndpointType,
    ) -> Result<<EndpointType as Endpoint>::Response, Error> {
        self.http_client.request(endpoint).await
    }

    pub fn base_url(&self) -> &url::Url {
        &self.base_url
    }

    pub fn api_key(&self) -> &http::HeaderValue {
        &self.api_key
    }
}

pub use get_address::GetAddress;
mod get_address {
    use super::*;
    use ethers_core::types::{Address, TxHash};

    #[derive(Clone, Debug)]
    pub struct GetAddress {
        pub address: Address,
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct GetAddressResponse {
        pub hash: Address,
        pub creation_transaction_hash: Option<TxHash>,
        pub creator_address_hash: Option<Address>,
        pub is_verified: Option<bool>,
        pub is_contract: bool,
    }

    impl Endpoint for GetAddress {
        type Response = GetAddressResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v2/addresses/{}", self.address.to_hex())
        }
    }
}

pub use get_smart_contract::GetSmartContract;
mod get_smart_contract {
    use super::*;
    use ethers_core::types::Address;
    use serde_with::serde_as;

    #[derive(Clone, Debug)]
    pub struct GetSmartContract {
        pub address: Address,
    }

    impl Endpoint for GetSmartContract {
        type Response = GetSmartContractResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v2/smart-contracts/{}", self.address.to_hex())
        }
    }

    #[serde_as]
    #[derive(Clone, Debug, Deserialize)]
    pub struct GetSmartContractResponse {
        #[serde_as(as = "Option<blockscout_display_bytes::serde_as::Hex>")]
        pub deployed_bytecode: Option<Vec<u8>>,
        #[serde_as(as = "Option<blockscout_display_bytes::serde_as::Hex>")]
        pub creation_bytecode: Option<Vec<u8>>,
    }
}

pub use get_transaction::GetTransaction;
mod get_transaction {
    use super::*;
    use ethers_core::types::TxHash;

    #[derive(Clone, Debug)]
    pub struct GetTransaction {
        pub hash: TxHash,
    }

    impl Endpoint for GetTransaction {
        type Response = GetTransactionResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("api/v2/transactions/{}", self.hash.to_hex())
        }
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct GetTransactionResponse {
        pub block_number: u128,
        pub position: u32,
    }
}

pub use import_smart_contract::ImportSmartContract;
mod import_smart_contract {
    use super::*;
    use ethers_core::types::Address;
    use http::{HeaderMap, HeaderValue};

    #[derive(Clone, Debug)]
    pub struct ImportSmartContract {
        pub address: Address,
        pub api_key: HeaderValue,
    }

    impl Endpoint for ImportSmartContract {
        type Response = ImportSmartContractResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v2/import/smart-contracts/{}", self.address.to_hex())
        }

        fn headers(&self) -> Option<HeaderMap> {
            let mut header_map = HeaderMap::new();
            header_map.insert(API_KEY_HEADER, self.api_key.clone());
            Some(header_map)
        }
    }

    #[derive(Clone, Debug, Deserialize)]
    pub struct ImportSmartContractResponse {
        pub message: String,
    }
}
