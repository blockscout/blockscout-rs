use std::time::Duration;

use self::disperser::{disperser_client::DisperserClient, RetrieveBlobRequest};
use anyhow::Result;
use tokio::time::sleep;
use tonic::{transport::Channel, Status};

mod disperser {
    #![allow(clippy::all)]
    tonic::include_proto!("disperser");
}

mod common {
    #![allow(clippy::all)]
    tonic::include_proto!("common");
}

pub struct Client {
    retry_delays: Vec<Duration>,
    client: DisperserClient<Channel>,
}

impl Client {
    pub async fn new(disperser_endpoint: &str, retry_delays: Vec<u64>) -> Result<Self> {
        let client = DisperserClient::connect(disperser_endpoint.to_string()).await?;
        let retry_delays = retry_delays.into_iter().map(Duration::from_secs).collect();
        Ok(Self {
            retry_delays,
            client,
        })
    }

    pub async fn retrieve_blob_with_retries(
        &self,
        batch_id: u64,
        batch_header_hash: &[u8],
        blob_index: u32,
    ) -> Result<Option<Vec<u8>>> {
        let mut last_err = Status::new(tonic::Code::Unknown, "Unknown error");
        for delay in self.retry_delays.iter() {
            match self
                .retrieve_blob(batch_id, batch_header_hash.to_vec(), blob_index)
                .await
            {
                Ok(blob) => return Ok(Some(blob)),
                Err(e) => {
                    // We use NotFound as a signal that previous blob was the last one
                    // since we don't know the blobs count beforehand
                    if e.code() == tonic::Code::NotFound {
                        return Ok(None);
                    }
                    tracing::warn!(
                        batch_id,
                        blob_index,
                        ?delay,
                        "failed to fetch blob: {}, retrying",
                        e
                    );
                    last_err = e;
                    sleep(*delay).await;
                }
            }
        }
        tracing::error!(
            batch_id,
            blob_index,
            "failed to fetch blob: {}, skipping the batch",
            last_err
        );
        Err(last_err.into())
    }

    async fn retrieve_blob(
        &self,
        batch_id: u64,
        batch_header_hash: Vec<u8>,
        blob_index: u32,
    ) -> Result<Vec<u8>, Status> {
        tracing::debug!(batch_id, blob_index, "fetching blob");
        let retrieve_request = RetrieveBlobRequest {
            batch_header_hash,
            blob_index,
        };
        let mut client = self.client.clone();
        client
            .retrieve_blob(retrieve_request)
            .await
            .map(|response| response.into_inner().data)
    }
}
