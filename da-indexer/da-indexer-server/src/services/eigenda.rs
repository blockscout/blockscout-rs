use std::str::FromStr;

use crate::proto::eigen_da_service_server::EigenDaService as EigenDa;
use base64::prelude::*;
use blockscout_display_bytes::Bytes;
use da_indexer_logic::eigenda::repository::blobs;
use da_indexer_proto::blockscout::da_indexer::v1::{EigenDaBlob, GetEigenDaBlobRequest};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct EigenDaService {
    db: DatabaseConnection,
}

impl EigenDaService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl EigenDa for EigenDaService {
    async fn get_blob(
        &self,
        request: Request<GetEigenDaBlobRequest>,
    ) -> Result<Response<EigenDaBlob>, Status> {
        let inner = request.into_inner();

        let blob_index = inner.blob_index;
        let batch_header_hash = Bytes::from_str(&inner.batch_header_hash)
            .map(|b| b.to_vec())
            .or_else(|_| BASE64_STANDARD.decode(&inner.batch_header_hash))
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to decode batch header hash");
                Status::invalid_argument("failed to decode batch header hash")
            })?;

        let blob = blobs::find(&self.db, &batch_header_hash, blob_index as i32)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query blob");
                Status::internal("failed to query blob")
            })?
            .ok_or(Status::not_found("blob not found"))?;

        let data =
            (!inner.skip_data.unwrap_or_default()).then_some(BASE64_STANDARD.encode(&blob.data));

        Ok(Response::new(EigenDaBlob {
            batch_header_hash: inner.batch_header_hash,
            batch_id: blob.batch_id as u64,
            blob_index: blob.blob_index as u32,
            l1_confirmation_block: blob.l1_block as u64,
            l1_confirmation_tx_hash: hex::encode(blob.l1_tx_hash),
            size: blob.data.len() as u64,
            data,
        }))
    }
}
