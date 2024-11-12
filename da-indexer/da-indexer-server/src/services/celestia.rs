use crate::proto::celestia_service_server::CelestiaService as Celestia;
use base64::prelude::*;
use da_indexer_logic::celestia::{l2_router::L2Router, repository::blobs};
use da_indexer_proto::blockscout::da_indexer::v1::{
    CelestiaBlob, CelestiaBlobId, CelestiaL2BatchMetadata, CelestiaNamespaces, Empty,
    GetCelestiaBlobRequest,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

use super::bytes_from_hex_or_base64;

#[derive(Default)]
pub struct CelestiaService {
    db: Option<DatabaseConnection>,
    l2_router: Option<L2Router>,
}

impl CelestiaService {
    pub fn new(db: Option<DatabaseConnection>, l2_router: Option<L2Router>) -> Self {
        Self { db, l2_router }
    }
}

#[async_trait::async_trait]
impl Celestia for CelestiaService {
    async fn get_blob(
        &self,
        request: Request<GetCelestiaBlobRequest>,
    ) -> Result<Response<CelestiaBlob>, Status> {
        let db = self
            .db
            .as_ref()
            .ok_or(Status::unimplemented("database is not configured"))?;
        let inner = request.into_inner();

        let height = inner.height;
        let commitment = bytes_from_hex_or_base64(&inner.commitment, "commitment")?;

        let blob = blobs::find_by_height_and_commitment(db, height, &commitment)
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

    async fn get_l2_batch_metadata(
        &self,
        request: Request<CelestiaBlobId>,
    ) -> Result<Response<CelestiaL2BatchMetadata>, Status> {
        let l2_router = self
            .l2_router
            .as_ref()
            .ok_or(Status::unimplemented("l2 router is not configured"))?;
        let inner = request.into_inner();

        let height = inner.height;
        let commitment = bytes_from_hex_or_base64(&inner.commitment, "commitment")?;
        let namespace = bytes_from_hex_or_base64(&inner.namespace, "namespace")?;

        let mut l2_batch_metadata = l2_router
            .get_l2_batch_metadata(height, &namespace, &commitment)
            .await
            .map_err(|err| {
                tracing::error!(height, namespace = hex::encode(&namespace), commitment = hex::encode(&commitment), error = ?err, "failed to query l2 batch metadata");
                Status::internal("failed to query l2 batch metadata")
            })?
            .ok_or(Status::not_found("l2 batch metadata not found"))?;

        let related_blobs = l2_batch_metadata
            .related_blobs
            .drain(..)
            .map(|blob| CelestiaBlobId {
                height: blob.height,
                namespace: blob.namespace,
                commitment: blob.commitment,
            })
            .collect();

        Ok(Response::new(CelestiaL2BatchMetadata {
            l2_chain_id: l2_batch_metadata.l2_chain_id,
            l2_batch_id: l2_batch_metadata.l2_batch_id,
            l2_start_block: l2_batch_metadata.l2_start_block,
            l2_end_block: l2_batch_metadata.l2_end_block,
            l2_blockscout_url: l2_batch_metadata.l2_blockscout_url,
            l2_batch_tx_count: l2_batch_metadata.l2_batch_tx_count,
            l1_tx_hash: l2_batch_metadata.l1_tx_hash,
            l1_tx_timestamp: l2_batch_metadata.l1_tx_timestamp,
            l1_chain_id: l2_batch_metadata.l1_chain_id,
            related_blobs,
        }))
    }

    async fn get_l2_namespaces(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<CelestiaNamespaces>, Status> {
        let l2_router = self
            .l2_router
            .as_ref()
            .ok_or(Status::unimplemented("l2 router is not configured"))?;

        let namespaces = l2_router.get_namespaces();
        Ok(Response::new(CelestiaNamespaces { namespaces }))
    }
}
