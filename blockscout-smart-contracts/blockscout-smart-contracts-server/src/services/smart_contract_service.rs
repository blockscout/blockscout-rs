use crate::proto::smart_contract_service_server::*;
use crate::proto::*;
use blockscout_smart_contracts_logic::address_utils::{format_address_hex_from_db, parse_address_to_bytes};
use blockscout_smart_contracts_logic::create_input::CreateInput;
use blockscout_smart_contracts_logic::smart_contract_repo::{select_contract, select_sources, upsert_contract};
use blockscout_smart_contracts_logic::ApiError;
use convert_trait::TryConvert;
use sea_orm::DatabaseConnection;
use std::collections::BTreeMap;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SmartContractServiceImpl {
    pub db: Arc<DatabaseConnection>,
}

#[async_trait::async_trait]
impl SmartContractService for SmartContractServiceImpl {
    async fn smart_contract_service_create(
        &self,
        request: Request<SmartContractServiceCreateRequest>,
    ) -> Result<Response<SmartContractServiceCreateResponse>, Status> {
        let (_metadata, _, req) = request.into_parts();

        let req_internal: SmartContractServiceCreateRequestInternal = TryConvert::try_convert(req)
            .map_err(ApiError::Convert)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let input = convert_request(req_internal)?;
        upsert_contract(&self.db, &input).await?;

        Ok(Response::new(SmartContractServiceCreateResponse {}))
    }

    async fn smart_contract_service_get(
        &self,
        request: Request<SmartContractServiceGetRequest>,
    ) -> Result<Response<SmartContractServiceGetResponse>, Status> {
        let req = request.into_inner();

        let addr_bytes: Vec<u8> = parse_address_to_bytes(req.address.as_str())?;

        let contract_opt = select_contract(self.db.as_ref(), req.chain_id.as_str(), addr_bytes).await?;
        let Some(record) = contract_opt else {
            return Ok(Response::new(SmartContractServiceGetResponse { contract: None }));
        };

        let address_hex = format_address_hex_from_db(&record.address_db, &req.address);

        let sources: BTreeMap<String, String> = select_sources(self.db.as_ref(), record.id).await?;
        let contract = SmartContract {
            chain_id: record.chain_id,
            address: address_hex,
            blockscout_url: record.blockscout_url,
            sources,
        };

        Ok(Response::new(SmartContractServiceGetResponse {
            contract: Some(contract),
        }))
    }
}

pub fn convert_request(req: SmartContractServiceCreateRequestInternal) -> Result<CreateInput, Status> {
    let contract = req
        .contract
        .ok_or_else(|| Status::invalid_argument("contract is required"))?;

    let address_bytes = parse_address_to_bytes(&contract.address)?;
    Ok(CreateInput {
        chain_id: contract.chain_id,
        address_bytes,
        blockscout_url: contract.blockscout_url.to_string(),
        sources: contract.sources,
    })
}
