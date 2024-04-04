use crate::proto::{
    cache_server::Cache, CreateSmartContractRequest, CreateSmartContractRequestInternal,
    GetSmartContractRequest, SmartContract,
};
use basic_cache_proto::blockscout::basic_cache::v1::GetSmartContractRequestInternal;
use convert_trait::TryConvert;

#[derive(Default)]
pub struct CacheService<I> {
    implementation: I,
}

impl<I> CacheService<I> {
    pub fn new(implementation: I) -> Self {
        Self { implementation }
    }
}

#[async_trait::async_trait]
impl<I> Cache for CacheService<I>
where
    I: basic_cache_logic::CacheManager<
            basic_cache_logic::types::SmartContractId,
            basic_cache_logic::types::SmartContractValue,
        > + Send
        + Sync
        + 'static,
{
    async fn create_smart_contract(
        &self,
        request: tonic::Request<CreateSmartContractRequest>,
    ) -> Result<tonic::Response<SmartContract>, tonic::Status> {
        let request = CreateSmartContractRequestInternal::try_convert(request.into_inner())
            .map_err(|err| tonic::Status::invalid_argument(format!("invalid request: {}", err)))?;
        let contract = basic_cache_logic::types::SmartContract::try_from(request)
            .map_err(|err| tonic::Status::invalid_argument(format!("invalid request: {}", err)))?;
        let existing_contract = self
            .implementation
            .replace(contract.id.clone(), contract.value.clone())
            .await
            .inspect_err(|err| tracing::error!("failed to store contract: {:?}", err))
            .map_err(|_| tonic::Status::internal("failed to store contract"))?;
        match existing_contract {
            Some(_) => tracing::info!("overwritten contract at {:?}", contract.id),
            None => tracing::info!("saved contract at {:?}", &contract.id),
        }
        Ok(tonic::Response::new(contract.value.into()))
    }

    async fn get_smart_contract(
        &self,
        request: tonic::Request<GetSmartContractRequest>,
    ) -> Result<tonic::Response<SmartContract>, tonic::Status> {
        let request = GetSmartContractRequestInternal::try_convert(request.into_inner())
            .map_err(|err| tonic::Status::invalid_argument(format!("invalid request: {}", err)))?;
        let contract_id = basic_cache_logic::types::SmartContractId::from(request);
        let contract = self
            .implementation
            .get(&contract_id)
            .await
            .inspect_err(|err| tracing::error!("failed to retrieve contract: {:?}", err))
            .map_err(|_| tonic::Status::internal("failed to retrieve contract"))?
            .ok_or(tonic::Status::not_found(
                "did not find contract with given chain id and address",
            ))?;
        Ok(tonic::Response::new(contract.into()))
    }
}
