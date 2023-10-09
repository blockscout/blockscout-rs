use crate::{
    proto::{
        solidity_visualizer_server::SolidityVisualizer, VisualizeContractsRequest,
        VisualizeResponse, VisualizeStorageRequest,
    },
    types::{
        VisualizeContractsRequestWrapper, VisualizeResponseWrapper, VisualizeStorageRequestWrapper,
    },
};
use async_trait::async_trait;

#[derive(Default)]
pub struct SolidityVisualizerService {}

#[async_trait]
impl SolidityVisualizer for SolidityVisualizerService {
    #[tracing::instrument(skip(self, request), level = "info")]
    async fn visualize_contracts(
        &self,
        request: tonic::Request<VisualizeContractsRequest>,
    ) -> Result<tonic::Response<VisualizeResponse>, tonic::Status> {
        let request: VisualizeContractsRequestWrapper = request.into_inner().into();
        let result = visualizer::visualize_contracts(request.try_into()?).await;
        result
            .map(|response| tonic::Response::new(VisualizeResponseWrapper::from(response).into()))
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
        let request: VisualizeStorageRequestWrapper = request.into_inner().into();
        let result = visualizer::visualize_storage(request.try_into()?).await;
        result
            .map(|response| tonic::Response::new(VisualizeResponseWrapper::from(response).into()))
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
