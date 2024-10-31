use crate::proto::{
    multichain_aggregator_service_server::MultichainAggregatorService, BatchImportRequest,
    BatchImportResponse,
};
use multichain_aggregator_logic::{
    self as logic, api_key_manager::ApiKeyManager, error::ServiceError,
};
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    db: DatabaseConnection,
    api_key_manager: ApiKeyManager,
}

impl MultichainAggregator {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db: db.clone(),
            api_key_manager: ApiKeyManager::new(db),
        }
    }
}

#[async_trait::async_trait]
impl MultichainAggregatorService for MultichainAggregator {
    async fn batch_import(
        &self,
        request: Request<BatchImportRequest>,
    ) -> Result<Response<BatchImportResponse>, Status> {
        let inner = request.into_inner();

        let api_key = (inner.api_key.as_str(), inner.chain_id.as_str())
            .try_into()
            .map_err(ServiceError::from)?;
        self.api_key_manager
            .validate_api_key(api_key)
            .await
            .map_err(ServiceError::from)?;

        let import_request: logic::BatchImportRequest = inner.try_into()?;

        logic::batch_import(&self.db, import_request)
            .await
            .map_err(|err| {
                tracing::error!(error = ?err, "failed to batch import");
                Status::internal("failed to batch import")
            })?;

        Ok(Response::new(BatchImportResponse {
            status: "ok".to_string(),
        }))
    }
}
