use std::sync::Arc;

use tac_operation_lifecycle_logic::database::TacDatabase;

use crate::proto::{
    tac_service_server::TacService, GetOperationStatusRequest, GetOperationStatusResponse
};

pub struct OperationsService {
    db: Arc<TacDatabase>,
}

impl OperationsService {
    pub fn new(db: Arc<TacDatabase>) -> Self {
        Self { db }
    }
}


#[async_trait::async_trait]
impl TacService for OperationsService {
    async fn get_operation_status(
        &self,
        request: tonic::Request<GetOperationStatusRequest>
    ) -> Result<tonic::Response<GetOperationStatusResponse>, tonic::Status> { 

        let inner = request.into_inner();

        // TODO: request operation status/stages
        match self.db.get_operations_with_stages(&inner.operation_id).await {
            Ok(Some(_res)) => {
                Ok(tonic::Response::new(GetOperationStatusResponse {
                    operation_id: inner.operation_id,
                    status_history: [].to_vec()
                }))
            },

            Ok(None) =>
                Err(tonic::Status::not_found("cannot find operation id")),

            Err(e) => Err(tonic::Status::internal(e.to_string()))
        }
    }
}
