use crate::proto::{
    cache_server::Cache, CreateSmartContractRequest, CreateSmartContractRequestInternal,
    GetSmartContractRequest, SmartContract, SourceFile,
};
use convert_trait::TryConvert;

#[derive(Default)]
pub struct CacheService<I> {
    implementation: I,
}

#[async_trait::async_trait]
impl<I> Cache for CacheService<I>
where
    I: basic_cache_logic::CacheManager<
            basic_cache_logic::types::SmartContractId,
            basic_cache_logic::types::SmartContract,
        > + Send
        + Sync
        + 'static,
{
    async fn create_smart_contract(
        &self,
        request: tonic::Request<CreateSmartContractRequest>,
    ) -> Result<tonic::Response<SmartContract>, tonic::Status> {
        let request = CreateSmartContractRequestInternal::try_convert(request.into_inner())
            .map_err(|err| {
                tonic::Status::invalid_argument(format!("invalid submission request: {}", err))
            })?;
        let contract =
            basic_cache_logic::types::SmartContract::try_from(request).map_err(|err| {
                tonic::Status::invalid_argument(format!("invalid submission request: {}", err))
            })?;
        Ok(tonic::Response::new(contract.into()))
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
