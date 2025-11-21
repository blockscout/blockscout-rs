use super::bytes_from_hex_or_base64;
use crate::proto::{
    eigen_da_service_server::EigenDaService as EigenDa,
    eigen_da_v2_service_server::EigenDaV2Service as EigenDaV2,
};
use base64::prelude::*;
use blockscout_display_bytes::ToHex;
use da_indexer_logic::{
    eigenda::{
        eigenda_proxy_client,
        repository::{blobs, blobs_v2},
        settings::EigendaV2ServerSettings,
    },
    s3_storage::S3Storage,
};
use da_indexer_proto::blockscout::da_indexer::v1::{
    EigenDaBlob, EigenDaV2Blob, GetEigenDaBlobRequest, GetEigenDaV2Blob,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

pub struct EigenDaService {
    db: Option<DatabaseConnection>,
    s3_storage: Option<S3Storage>,
    v2_proxy_client: eigenda_proxy_client::Client,
}

impl EigenDaService {
    pub fn new(
        db: Option<DatabaseConnection>,
        s3_storage: Option<S3Storage>,
        v2_server_settings: Option<EigendaV2ServerSettings>,
    ) -> Self {
        let v2_proxy_client =
            eigenda_proxy_client::Client::new(v2_server_settings.unwrap_or_default());
        Self {
            db,
            s3_storage,
            v2_proxy_client,
        }
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

#[async_trait::async_trait]
impl EigenDaV2 for EigenDaService {
    async fn get_blob(
        &self,
        request: Request<GetEigenDaV2Blob>,
    ) -> Result<Response<EigenDaV2Blob>, Status> {
        let db = self
            .db
            .as_ref()
            .ok_or(Status::unimplemented("database is not configured"))?;
        let inner = request.into_inner();

        let commitment = bytes_from_hex_or_base64(&inner.commitment, "commitment")?;
        let maybe_blob = blobs_v2::find_by_commitment(db, self.s3_storage.as_ref(), &commitment)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to query blob from the database");
                Status::internal("failed to query blob")
            })?;

        if let Some(blob) = maybe_blob {
            return Ok(Response::new(EigenDaV2Blob {
                data: blob.to_hex(),
            }));
        }

        let proxy_base_url = parse_required_url("proxy_base_url", inner.proxy_base_url)?;
        let maybe_proxy_blob = self
            .v2_proxy_client
            .request_blob(proxy_base_url, &commitment)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to retrieve blob via the proxy");
                Status::internal("failed to query blob")
            })?;

        match maybe_proxy_blob {
            None => Err(Status::not_found("blob not found")),
            Some(proxy_blob) => {
                blobs_v2::insert_commitment_with_data(
                    db,
                    self.s3_storage.as_ref(),
                    &commitment,
                    proxy_blob.clone(),
                )
                .await
                .map_err(|err| {
                    tracing::error!(error = ?err, "failed to insert proxy blob into storage");
                    Status::internal("failed to store retrieved blob into storage")
                })?;

                Ok(Response::new(EigenDaV2Blob {
                    data: proxy_blob.to_hex(),
                }))
            }
        }
    }
}

fn parse_required_url(
    field: &'static str,
    maybe_string: Option<String>,
) -> Result<url::Url, Status> {
    match maybe_string {
        None => Err(Status::invalid_argument(format!("{field} is missing"))),
        Some(string) => url::Url::parse(&string)
            .map_err(|err| Status::invalid_argument(format!("{field} is invalid url: {err}"))),
    }
}
