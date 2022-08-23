use tonic::{transport::Server, Request, Response, Status};

mod proto;
use proto::{
    smart_contract_service_server::{SmartContractService, SmartContractServiceServer},
    *,
};

#[derive(Default)]
pub struct SmartContract {}

#[async_trait::async_trait]
impl SmartContractService for SmartContract {
    async fn get_abi(
        &self,
        request: tonic::Request<GetAbiRequest>,
    ) -> Result<tonic::Response<GetAbiResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn get_sources(
        &self,
        request: tonic::Request<GetSourcesRequest>,
    ) -> Result<tonic::Response<GetSourcesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn verify_single(
        &self,
        request: tonic::Request<VerifySingleRequest>,
    ) -> Result<tonic::Response<VerifySingleResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn verify_multi(
        &self,
        request: tonic::Request<VerifyMultiRequest>,
    ) -> Result<tonic::Response<VerifyMultiResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn verify_standard_json(
        &self,
        request: tonic::Request<VerifyStandardJsonRequest>,
    ) -> Result<tonic::Response<VerifyStandardJsonResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn verify_via_sourcify(
        &self,
        request: tonic::Request<VerifyViaSourcifyRequest>,
    ) -> Result<tonic::Response<VerifyViaSourcifyResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }

    async fn verify_vyper(
        &self,
        request: tonic::Request<VerifyVyperRequest>,
    ) -> Result<tonic::Response<VerifyVyperResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented("not implemented"))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let greeter = SmartContract::default();

    Server::builder()
        .add_service(SmartContractServiceServer::new(greeter))
        .serve(addr)
        .await?;

    Ok(())
}
