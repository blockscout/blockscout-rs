mod http_client;

#[cfg(test)]
mod http_client_test;

/************************************************************/

use alloy_rpc_types_beacon::{
    header::HeaderResponse,
    sidecar::{BeaconBlobBundle, GetBlobsResponse},
};
use api_client_framework::Error;
use async_trait::async_trait;

#[async_trait]
pub trait BeaconBlockHeadersProvider {
    async fn beacon_block_header(&self, block_id: String) -> Result<HeaderResponse, Error>;
}

#[async_trait]
pub trait BlobSidecarsProvider {
    async fn blob_sidecars(&self, block_id: String) -> Result<BeaconBlobBundle, Error>;
}

#[async_trait]
pub trait BlobsProvider {
    async fn blobs(&self, block_id: String) -> Result<GetBlobsResponse, Error>;
}

pub trait Client: BeaconBlockHeadersProvider + BlobSidecarsProvider + BlobsProvider {}
