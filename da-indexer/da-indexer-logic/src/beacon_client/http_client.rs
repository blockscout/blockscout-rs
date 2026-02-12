use alloy_rpc_types_beacon::{
    header::HeaderResponse,
    sidecar::{BeaconBlobBundle, GetBlobsResponse},
};
use api_client_framework::Error;
use std::time::Duration;
use tonic::async_trait;

pub struct HttpClientSettings {
    pub beacon_url: url::Url,
    pub timeout: Duration,
}

pub struct HttpClient {
    client: api_client_framework::HttpApiClient,
}

impl HttpClient {
    pub fn new(settings: HttpClientSettings) -> Self {
        let config = api_client_framework::HttpApiClientConfig {
            http_timeout: settings.timeout,
            default_headers: Default::default(),
            middlewares: vec![],
        };
        let client = api_client_framework::HttpApiClient::new(settings.beacon_url, config)
            .expect("failed to initialize client");
        Self { client }
    }
}

#[async_trait]
impl super::BeaconBlockHeadersProvider for HttpClient {
    async fn beacon_block_header(&self, block_id: String) -> Result<HeaderResponse, Error> {
        self.client
            .request(&endpoints::GetBeaconBlockHeader { block_id })
            .await
    }
}

#[async_trait]
impl super::BlobSidecarsProvider for HttpClient {
    async fn blob_sidecars(&self, block_id: String) -> Result<BeaconBlobBundle, Error> {
        self.client
            .request(&endpoints::GetBlobSidecars { block_id })
            .await
    }
}

#[async_trait]
impl super::BlobsProvider for HttpClient {
    async fn blobs(&self, block_id: String) -> Result<GetBlobsResponse, Error> {
        self.client.request(&endpoints::GetBlobs { block_id }).await
    }
}

impl super::Client for HttpClient {}

mod endpoints {
    use super::*;
    use api_client_framework::Endpoint;
    use http::Method;

    pub struct GetBeaconBlockHeader {
        pub block_id: String,
    }

    impl Endpoint for GetBeaconBlockHeader {
        type Response = HeaderResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("eth/v1/beacon/headers/{}", self.block_id)
        }
    }

    pub struct GetBlobSidecars {
        pub block_id: String,
    }

    impl Endpoint for GetBlobSidecars {
        type Response = BeaconBlobBundle;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/eth/v1/beacon/blob_sidecars/{}", self.block_id)
        }
    }

    pub struct GetBlobs {
        pub block_id: String,
    }

    impl Endpoint for GetBlobs {
        type Response = GetBlobsResponse;

        fn method(&self) -> Method {
            Method::GET
        }

        fn path(&self) -> String {
            format!("/eth/v1/beacon/blobs/{}", self.block_id)
        }
    }
}
