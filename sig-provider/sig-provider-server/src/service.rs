use async_trait::async_trait;
use ethabi::{ethereum_types::H256, RawLog};
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

fn decode(str: &str) -> Result<Vec<u8>, tonic::Status> {
    hex::decode(str.strip_prefix("0x").unwrap_or(str))
        .map_err(|e| tonic::Status::invalid_argument(e.to_string()))
}

#[async_trait]
impl AbiService for Service {
    async fn get_function_abi(
        &self,
        request: tonic::Request<GetFunctionAbiRequest>,
    ) -> Result<tonic::Response<GetFunctionAbiResponse>, tonic::Status> {
        let request = request.into_inner();
        let bytes = decode(&request.tx_input)?;
        self.agg
            .get_function_abi(&bytes)
            .await
            .map(|abi| tonic::Response::new(GetFunctionAbiResponse { abi }))
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }

    async fn get_event_abi(
        &self,
        request: tonic::Request<GetEventAbiRequest>,
    ) -> Result<tonic::Response<GetEventAbiResponse>, tonic::Status> {
        let request = request.into_inner();
        let topics: Result<Vec<_>, _> = request
            .topics
            .into_iter()
            .map(|topic| {
                let hex = decode(&topic)?;
                if hex.len() != 32 {
                    return Err(tonic::Status::invalid_argument(
                        "topic len must be 32 bytes",
                    ));
                }
                Ok(H256::from_slice(&hex))
            })
            .collect();
        let topics = topics?;
        self.agg
            .get_event_abi(RawLog {
                data: decode(&request.data)?,
                topics,
            })
            .await
            .map(|abi| tonic::Response::new(GetEventAbiResponse { abi }))
            .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}
