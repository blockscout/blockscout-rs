use crate::{
    conversion::{self, ConversionError},
    proto::{metadata_server::Metadata, GetMetadataRequest, GetMetadataResponse},
};
use async_trait::async_trait;
use metadata_logic::get_metadata;
use sea_orm::DatabaseConnection;

pub struct MetadataService {
    conn: DatabaseConnection,
}

impl MetadataService {
    pub fn new(conn: DatabaseConnection) -> Self {
        Self { conn }
    }
}

#[async_trait]
impl Metadata for MetadataService {
    async fn get_metadata(
        &self,
        request: tonic::Request<GetMetadataRequest>,
    ) -> Result<tonic::Response<GetMetadataResponse>, tonic::Status> {
        let request = conversion::get_metadata_request_from_inner(request.into_inner())
            .map_err(map_conversion_error)?;
        let res = get_metadata(&self.conn, request)
            .await
            .map_err(|err| tonic::Status::internal(err.to_string()))
            .map(conversion::get_metadata_response_from_logic)?
            .map_err(map_conversion_error)?;
        Ok(tonic::Response::new(res))
    }
}

fn map_conversion_error(err: ConversionError) -> tonic::Status {
    match err {
        ConversionError::UserRequest(_) => tonic::Status::invalid_argument(err.to_string()),
        ConversionError::LogicOutput(_) => tonic::Status::internal(err.to_string()),
    }
}
