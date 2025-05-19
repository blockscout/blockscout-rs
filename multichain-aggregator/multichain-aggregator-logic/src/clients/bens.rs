use api_client_framework::{Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig};
use reqwest::Method;
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod lookup_domain_name {
    use super::*;
    use api_client_framework::serialize_query;
    use bens_proto::blockscout::bens::v1::{
        LookupDomainNameRequest, LookupDomainNameResponse, Order,
    };
    use serde::Serialize;

    pub struct LookupDomainName {
        pub request: LookupDomainNameRequest,
    }

    impl Endpoint for LookupDomainName {
        type Response = LookupDomainNameResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v1/{}/domains:lookup", self.request.chain_id)
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            pub struct Params<'a> {
                pub name: Option<&'a String>,
                pub chain_id: i64,
                pub only_active: bool,
                pub sort: &'a String,
                pub order: Order,
                pub page_size: Option<u32>,
                pub page_token: Option<&'a String>,
                pub protocols: Option<&'a String>,
            }

            let params = Params {
                name: self.request.name.as_ref(),
                chain_id: self.request.chain_id,
                only_active: self.request.only_active,
                sort: &self.request.sort,
                order: self
                    .request
                    .order
                    .try_into()
                    .expect("valid order enum should be provided"),
                page_size: self.request.page_size,
                page_token: self.request.page_token.as_ref(),
                protocols: self.request.protocols.as_ref(),
            };
            serialize_query(&params)
        }
    }
}

pub mod get_address {
    use super::*;
    use api_client_framework::serialize_query;
    use bens_proto::blockscout::bens::v1::{GetAddressRequest, GetAddressResponse};
    use serde::Serialize;

    pub struct GetAddress {
        pub request: GetAddressRequest,
    }

    impl Endpoint for GetAddress {
        type Response = GetAddressResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!(
                "/api/v1/{}/addresses/{}",
                self.request.chain_id, self.request.address
            )
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            pub struct Params<'a> {
                pub protocol_id: Option<&'a String>,
            }

            let params = Params {
                protocol_id: self.request.protocol_id.as_ref(),
            };
            serialize_query(&params)
        }
    }
}
