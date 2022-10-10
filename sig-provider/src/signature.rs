use crate::proto::blockscout::sig_provider::v1::{
    signature_service_server::SignatureService, CreateSignaturesRequest, CreateSignaturesResponse,
    GetSignaturesRequest, GetSignaturesResponse,
};

#[derive(Default)]
pub struct SignatureServer {}

#[async_trait::async_trait]
impl SignatureService for SignatureServer {
    async fn create_signatures(
        &self,
        request: tonic::Request<CreateSignaturesRequest>,
    ) -> Result<tonic::Response<CreateSignaturesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(""))
    }
    async fn get_function_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(""))
    }
    async fn get_event_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(""))
    }
    async fn get_error_signatures(
        &self,
        request: tonic::Request<GetSignaturesRequest>,
    ) -> Result<tonic::Response<GetSignaturesResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(""))
    }
}
