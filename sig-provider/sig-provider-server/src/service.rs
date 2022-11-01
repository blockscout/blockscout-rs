use async_trait::async_trait;
use sig_provider::SourceAggregator;
use sig_provider_proto::blockscout::sig_provider::v1::{
    abi_service_server::AbiService, signature_service_server::SignatureService,
    CreateSignaturesRequest, CreateSignaturesResponse, GetEventAbiRequest, GetEventAbiResponse,
    GetFunctionAbiRequest, GetFunctionAbiResponse,
};
use std::sync::Arc;

pub struct Service {
    agg: Arc<SourceAggregator>,
}

impl Service {
    pub fn new(agg: Arc<SourceAggregator>) -> Self {
        Self { agg }
    }
}

#[async_trait]
impl SignatureService for Service {
    async fn create_signatures(
        &self,
        request: tonic::Request<CreateSignaturesRequest>,
    ) -> Result<tonic::Response<CreateSignaturesResponse>, tonic::Status> {
        let request = request.into_inner();
        let agg = self.agg.clone();
        tokio::spawn(async move {
            let _result = agg.create_signatures(&request.abi).await;
        });
        Ok(tonic::Response::new(CreateSignaturesResponse {}))
    }
}

#[async_trait]
impl AbiService for Service {
    async fn get_function_abi(
        &self,
        request: tonic::Request<GetFunctionAbiRequest>,
    ) -> Result<tonic::Response<GetFunctionAbiResponse>, tonic::Status> {
        let request = request.into_inner();
        let bytes = hex::decode(
            request
                .tx_input
                .strip_prefix("0x")
                .unwrap_or(&request.tx_input),
        )
        .map_err(|e| tonic::Status::invalid_argument(e.to_string()))?;
        self.agg
            .get_function_abi(&bytes)
            .await
            .map(|abi| tonic::Response::new(GetFunctionAbiResponse { abi: Some(abi) }))
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }

    async fn get_event_abi(
        &self,
        request: tonic::Request<GetEventAbiRequest>,
    ) -> Result<tonic::Response<GetEventAbiResponse>, tonic::Status> {
        let request = request.into_inner();
        self.agg
            .get_event_abi(request.data, request.topics)
            .await
            .map(|abi| tonic::Response::new(GetEventAbiResponse { abi: Some(abi) }))
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}
