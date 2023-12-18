use async_trait::async_trait;
use ethabi::{ethereum_types::H256, RawLog};
use sig_provider::SourceAggregator;
use sig_provider_proto::blockscout::sig_provider::v1::{
    abi_service_server::AbiService, signature_service_server::SignatureService, Abi,
    BatchGetEventAbisRequest, BatchGetEventAbisResponse, CreateSignaturesRequest,
    CreateSignaturesResponse, GetEventAbiRequest, GetEventAbiResponse, GetFunctionAbiRequest,
    GetFunctionAbiResponse,
};
use std::{future::Future, sync::Arc};

#[derive(Clone)]
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
        let process_function = |raw| async { self.agg.get_event_abi(raw).await };
        self.get_event_abi_internal(request, process_function)
            .await
            .map(tonic::Response::new)
    }

    async fn batch_get_event_abis(
        &self,
        request: tonic::Request<BatchGetEventAbisRequest>,
    ) -> Result<tonic::Response<BatchGetEventAbisResponse>, tonic::Status> {
        let request = request.into_inner();

        let process_function = |raw| async { self.agg.batch_get_event_abi(raw).await };

        let mut responses = Vec::new();
        for event_request in request.requests {
            let response = self
                .get_event_abi_internal(event_request, process_function)
                .await?;
            responses.push(response);
        }
        Ok(tonic::Response::new(BatchGetEventAbisResponse {
            responses,
        }))
    }
}

impl Service {
    async fn get_event_abi_internal<F, Output>(
        &self,
        request: GetEventAbiRequest,
        process_function: F,
    ) -> Result<GetEventAbiResponse, tonic::Status>
    where
        F: Fn(RawLog) -> Output,
        Output: Future<Output = Result<Vec<Abi>, anyhow::Error>>,
    {
        let topics = parse_topics(request.topics)?;
        process_function(RawLog {
            data: decode(&request.data)?,
            topics,
        })
        .await
        .map(|abi| GetEventAbiResponse { abi })
        .map_err(|e| tonic::Status::internal(e.to_string()))
    }
}

fn parse_topics(topics: String) -> Result<Vec<H256>, tonic::Status> {
    topics
        .split(',')
        .map(|topic| {
            let hex = decode(topic)?;
            if hex.len() != 32 {
                return Err(tonic::Status::invalid_argument(
                    "topic len must be 32 bytes",
                ));
            }
            Ok(H256::from_slice(&hex))
        })
        .collect()
}
