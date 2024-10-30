use crate::proto::{
    multichain_aggregator_service_server::MultichainAggregatorService, BatchImportRequest,
    BatchImportResponse,
};
use multichain_aggregator_logic as logic;
use sea_orm::DatabaseConnection;
use tonic::{Request, Response, Status};

pub struct MultichainAggregator {
    db: DatabaseConnection,
}

impl MultichainAggregator {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait::async_trait]
impl MultichainAggregatorService for MultichainAggregator {
    async fn batch_import(
        &self,
        request: Request<BatchImportRequest>,
    ) -> Result<Response<BatchImportResponse>, Status> {
        let inner = request.into_inner();
        let import_request: logic::BatchImportRequest = inner.try_into().unwrap();

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
