use api_client_framework::{Endpoint, Error, HttpApiClient as Client, HttpApiClientConfig};
use reqwest::Method;
use url::Url;

pub fn new_client(url: Url) -> Result<Client, Error> {
    let config = HttpApiClientConfig::default();
    Client::new(url, config)
}

pub mod lookup_domain_name_multichain {
    use super::*;
    use api_client_framework::serialize_query;
    use bens_proto::blockscout::bens::v1::{
        LookupDomainNameMultichainRequest, LookupDomainNameResponse, Order,
    };
    use serde::Serialize;

    pub struct LookupDomainNameMultichain {
        pub request: LookupDomainNameMultichainRequest,
    }

    impl Endpoint for LookupDomainNameMultichain {
        type Response = LookupDomainNameResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/domains:lookup".to_string()
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            pub struct Params<'a> {
                pub name: Option<&'a String>,
                pub chain_id: Option<i64>,
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

pub mod get_address_multichain {
    use super::*;
    use api_client_framework::serialize_query;
    use bens_proto::blockscout::bens::v1::{GetAddressMultichainRequest, GetAddressResponse};
    use serde::Serialize;

    pub struct GetAddressMultichain {
        pub request: GetAddressMultichainRequest,
    }

    impl Endpoint for GetAddressMultichain {
        type Response = GetAddressResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v1/addresses/{}", self.request.address)
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            pub struct Params<'a> {
                pub chain_id: Option<i64>,
                pub protocols: Option<&'a String>,
            }

            let params = Params {
                chain_id: self.request.chain_id,
                protocols: self.request.protocols.as_ref(),
            };
            serialize_query(&params)
        }
    }
}

pub mod get_protocols {
    use super::*;
    use bens_proto::blockscout::bens::v1::{GetProtocolsRequest, GetProtocolsResponse};

    pub struct GetProtocols {
        pub request: GetProtocolsRequest,
    }

    impl Endpoint for GetProtocols {
        type Response = GetProtocolsResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/api/v1/{}/protocols", self.request.chain_id)
        }
    }
}

pub mod lookup_address_multichain {
    use super::*;
    use api_client_framework::serialize_query;
    use bens_proto::blockscout::bens::v1::{
        LookupAddressMultichainRequest, LookupAddressResponse, Order,
    };
    use serde::Serialize;

    pub struct LookupAddressMultichain {
        pub request: LookupAddressMultichainRequest,
    }

    impl Endpoint for LookupAddressMultichain {
        type Response = LookupAddressResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            "/api/v1/addresses:lookup".to_string()
        }

        fn query(&self) -> Option<String> {
            #[derive(Serialize)]
            pub struct Params<'a> {
                pub address: &'a String,
                pub chain_id: Option<i64>,
                pub protocols: Option<&'a String>,
                pub resolved_to: bool,
                pub owned_by: bool,
                pub only_active: bool,
                pub sort: &'a String,
                pub order: Order,
                pub page_size: Option<u32>,
                pub page_token: Option<&'a String>,
            }

            let params = Params {
                address: &self.request.address,
                chain_id: self.request.chain_id,
                protocols: self.request.protocols.as_ref(),
                resolved_to: self.request.resolved_to,
                owned_by: self.request.owned_by,
                only_active: self.request.only_active,
                sort: &self.request.sort,
                order: self
                    .request
                    .order
                    .try_into()
                    .expect("valid order enum should be provided"),
                page_size: self.request.page_size,
                page_token: self.request.page_token.as_ref(),
            };
            serialize_query(&params)
        }
    }
}
