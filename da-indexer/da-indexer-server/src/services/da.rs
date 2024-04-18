use std::str::FromStr;

use crate::proto::da_service_server::DaService as Da;
use base64::prelude::*;
use blockscout_display_bytes::Bytes;
use da_indexer_logic::celestia::repository::blobs;
use da_indexer_proto::blockscout::da_indexer::v1::{Blob, GetCelestiaBlobRequest};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct DaService {
    db: DatabaseConnection,
}

impl DaService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl Da for DaService {
    async fn get_celestia_blob(
        &self,
        request: Request<GetCelestiaBlobRequest>,
    ) -> Result<Response<Blob>, Status> {
        let inner = request.into_inner();

        let height = inner.height;
        let commitment = Bytes::from_str(&inner.commitment)
            .map(|b| b.to_vec())
            .or_else(|_| BASE64_STANDARD.decode(&inner.commitment))
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to decode commitment");
                Status::invalid_argument("failed to decode commitment")
            })?;

        let blob = blobs::find_by_height_and_commitment(&self.db, height, &commitment)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query blob");
                Status::internal("failed to query blob")
            })?
            .ok_or(Status::not_found("blob not found"))?;

        let data =
            (!inner.skip_data.unwrap_or_default()).then_some(BASE64_STANDARD.encode(&blob.data));

        Ok(Response::new(Blob {
            height: blob.height as u64,
            namespace: hex::encode(blob.namespace),
            commitment: inner.commitment,
            timestamp: blob.timestamp as u64,
            size: blob.data.len() as u64,
            data,
        }))
    }
}
