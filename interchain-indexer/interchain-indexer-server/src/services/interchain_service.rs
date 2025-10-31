use crate::proto::{interchain_service_server::*, *};
use convert_trait::TryConvert;
use interchain_indexer_logic::ApiError;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct InterchainServiceImpl {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait::async_trait]
impl InterchainService for InterchainServiceImpl {
    async fn interchain_service_create(
        &self,
        request: Request<InterchainServiceCreateRequest>,
    ) -> Result<Response<InterchainServiceCreateResponse>, Status> {
        let (_metadata, _, request) = request.into_parts();
        let _request: InterchainServiceCreateRequestInternal =
            TryConvert::try_convert(request).map_err(ApiError::Convert)?;
        todo!()
    }

    async fn interchain_service_search(
        &self,
        _request: Request<InterchainServiceSearchRequest>,
    ) -> Result<Response<InterchainServiceSearchResponse>, Status> {
        let items = (0..10)
            .map(|i| Item {
                id: i.to_string(),
                name: format!("Item #{}", i),
            })
            .collect();
        let response = InterchainServiceSearchResponse { items };
        Ok(Response::new(response))
    }
}
