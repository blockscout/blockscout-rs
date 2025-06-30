use super::bytes_from_hex_or_base64;
use crate::proto::eigen_da_service_server::EigenDaService as EigenDa;
use base64::prelude::*;
use da_indexer_logic::{eigenda::repository::blobs, s3_storage::S3Storage};
use da_indexer_proto::blockscout::da_indexer::v1::{EigenDaBlob, GetEigenDaBlobRequest};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

#[derive(Default)]
pub struct EigenDaService {
    db: Option<DatabaseConnection>,
    s3_storage: Option<S3Storage>,
}

impl EigenDaService {
    pub fn new(db: Option<DatabaseConnection>, s3_storage: Option<S3Storage>) -> Self {
        Self { db, s3_storage }
    }
}

#[async_trait::async_trait]
impl EigenDa for EigenDaService {
    async fn get_blob(
        &self,
        request: Request<GetEigenDaBlobRequest>,
    ) -> Result<Response<EigenDaBlob>, Status> {
        let db = self
            .db
            .as_ref()
            .ok_or(Status::unimplemented("database is not configured"))?;
        let inner = request.into_inner();

        let blob_index = inner.blob_index;
        let batch_header_hash =
            bytes_from_hex_or_base64(&inner.batch_header_hash, "batch header hash")?;

        let blob = blobs::find(
            db,
            self.s3_storage.as_ref(),
            &batch_header_hash,
            blob_index as i32,
        )
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
            l1_confirmation_tx_hash: format!("0x{}", hex::encode(blob.l1_tx_hash)),
            size: blob.data.len() as u64,
            data,
        }))
    }
}
