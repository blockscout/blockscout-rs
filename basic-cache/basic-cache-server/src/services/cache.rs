use crate::proto::{
    cache_server::Cache, CreateSmartContractRequest, GetSmartContractRequest, SmartContract,
    SourceFile,
};

#[derive(Default)]
pub struct CacheService {}

#[async_trait::async_trait]
impl Cache for CacheService {
    async fn create_smart_contract(
        &self,
        request: tonic::Request<CreateSmartContractRequest>,
    ) -> Result<tonic::Response<SmartContract>, tonic::Status> {
        let contract = request
            .get_ref()
            .smart_contract
            .clone()
            .ok_or(tonic::Status::new(
                tonic::Code::InvalidArgument,
                "Smart contract is required",
            ))?;
        Ok(tonic::Response::new(contract))
    }

    async fn get_smart_contract(
        &self,
        _request: tonic::Request<GetSmartContractRequest>,
    ) -> Result<tonic::Response<SmartContract>, tonic::Status> {
        Ok(tonic::Response::new(SmartContract {
            url: "some_contract_haha".into(),
            sources: vec![],
        }))
    }
}
