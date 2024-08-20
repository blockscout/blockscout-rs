use std::str::FromStr;

use crate::proto::celestia_service_server::CelestiaService as Celestia;
use base64::prelude::*;
use blockscout_display_bytes::Bytes;
use da_indexer_logic::celestia::repository::blobs;
use da_indexer_proto::blockscout::da_indexer::v1::{CelestiaBlob, GetCelestiaBlobRequest};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct CelestiaService {
    db: DatabaseConnection,
}

impl CelestiaService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl Celestia for CelestiaService {
    async fn get_blob(
        &self,
        request: Request<GetCelestiaBlobRequest>,
    ) -> Result<Response<CelestiaBlob>, Status> {
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

        Ok(Response::new(CelestiaBlob {
            height: blob.height as u64,
            namespace: hex::encode(blob.namespace),
            commitment: inner.commitment,
            timestamp: blob.timestamp as u64,
            size: blob.data.len() as u64,
            data,
        }))
    }
}
