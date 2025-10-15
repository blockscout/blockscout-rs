use crate::proto::interchain_service_server::*;
use crate::proto::*;
use tonic::{Request, Response, Status};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use convert_trait::TryConvert;
use interchain_indexer_logic::ApiError;
use interchain_indexer_logic::plus;

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
        let request: InterchainServiceCreateRequestInternal = TryConvert::try_convert(request).map_err(ApiError::Convert)?;
        todo!()
    }

    async fn interchain_service_search(
        &self,
        request: Request<InterchainServiceSearchRequest>,
    ) -> Result<Response<InterchainServiceSearchResponse>, Status> {
        let items = (0..10).map(|i| {
            let id = plus(i, i); 
            Item {
                id: id.to_string(),
                name: format!("Item #{}", id),
            }
        }).collect();
        let response = InterchainServiceSearchResponse {
            items,
        };
        Ok(Response::new(response))
    }
}
