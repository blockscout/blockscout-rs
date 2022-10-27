use crate::proto::blockscout::visualizer::v1::{
    solidity_visualizer_server::SolidityVisualizer, VisualizeContractsRequest, VisualizeResponse,
    VisualizeStorageRequest,
};

#[derive(Default)]
pub struct SolidityVisualizerService {}

#[async_trait::async_trait]
impl SolidityVisualizer for SolidityVisualizerService {
    #[tracing::instrument(skip(self, request), level = "info")]
    async fn visualize_contracts(
        &self,
        request: tonic::Request<VisualizeContractsRequest>,
    ) -> Result<tonic::Response<VisualizeResponse>, tonic::Status> {
        let request = visualizer::VisualizeContractsRequest::try_from(request.into_inner())
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let result = visualizer::visualize_contracts(request).await;
        result
            .map(|response| tonic::Response::new(response.into()))
            .map_err(|error| match error {
                visualizer::VisualizeContractsError::Internal(e) => {
                    tonic::Status::internal(e.to_string())
                }
                visualizer::VisualizeContractsError::Execution(e) => {
                    tonic::Status::invalid_argument(e)
                }
            })
    }

    #[tracing::instrument(skip(self, request), level = "info")]
    async fn visualize_storage(
        &self,
        request: tonic::Request<VisualizeStorageRequest>,
    ) -> Result<tonic::Response<VisualizeResponse>, tonic::Status> {
        let request = visualizer::VisualizeStorageRequest::try_from(request.into_inner())
            .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        let result = visualizer::visualize_storage(request).await;
        result
            .map(|response| tonic::Response::new(response.into()))
            .map_err(|error| match error {
                visualizer::VisualizeStorageError::Internal(e) => {
                    tonic::Status::internal(e.to_string())
                }
                visualizer::VisualizeStorageError::InvalidFileName => {
                    tonic::Status::invalid_argument("Invalid file name")
                }
                visualizer::VisualizeStorageError::Execution(e) => {
                    tonic::Status::invalid_argument(e)
                }
            })
    }
}
