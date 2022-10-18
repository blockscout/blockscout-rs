pub mod fourbyte;
pub mod sigeth;

use sig_provider_proto::blockscout::sig_provider::v1::{
    CreateSignaturesRequest, CreateSignaturesResponse, GetSignaturesRequest, GetSignaturesResponse,
};

#[async_trait::async_trait]
pub trait SignatureSource {
    async fn create_signatures(
        &self,
        request: CreateSignaturesRequest,
    ) -> Result<CreateSignaturesResponse, anyhow::Error>;
    async fn get_function_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error>;
    async fn get_event_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error>;
    async fn get_error_signatures(
        &self,
        request: GetSignaturesRequest,
    ) -> Result<GetSignaturesResponse, anyhow::Error>;

    // for errors
    fn host(&self) -> String;
}
