use crate::proto::smart_contract_service_server::*;
use crate::proto::*;
use tonic::{Request, Response, Status};
use sea_orm::{DatabaseConnection, TransactionTrait, ConnectionTrait};
use sea_orm::sea_query::Value;
use sea_orm::{DatabaseBackend, Statement, TryGetable};
use std::sync::Arc;
use std::str::FromStr;
use std::collections::BTreeMap;
use convert_trait::TryConvert;
use blockscout_smart_contracts_logic::ApiError;
use blockscout_smart_contracts_logic::plus;
use alloy_primitives::Address;

pub struct SmartContractServiceImpl {
    pub db: Arc<DatabaseConnection>,
    }

#[async_trait::async_trait]
impl SmartContractService for SmartContractServiceImpl {
    async fn smart_contract_service_create(
        &self,
        request: Request<SmartContractServiceCreateRequest>,
    ) -> Result<Response<SmartContractServiceCreateResponse>, Status> {
        let (_metadata, _, request) = request.into_parts();
        let request: SmartContractServiceCreateRequestInternal =
            TryConvert::try_convert(request)
                .map_err(ApiError::Convert)
                .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Persist SmartContract and its sources in a single transaction
        let txn = self.db.begin().await.map_err(|e| Status::internal(e.to_string()))?;

        let contract = request.contract.unwrap();

        let chain_id = contract.chain_id;

        // Convert to alloy_primitives::Address from hex string
        // Accepts 0x-prefixed or plain hex; will return InvalidArgument on parse failure
        let addr = Address::from_str(contract.address.as_str())
            .map_err(|e| Status::invalid_argument(format!("invalid contract.address: {e}")))?;
        let address_bytes: Vec<u8> = addr.as_slice().to_vec();

        let blockscout_url = contract.blockscout_url.to_string();

        // Insert or update the smart_contracts row and return its id
        let insert_contract_stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            INSERT INTO smart_contracts (chain_id, address, blockscout_url)
            VALUES ($1, $2, $3)
            ON CONFLICT (chain_id, address)
            DO UPDATE SET blockscout_url = EXCLUDED.blockscout_url
            RETURNING id
            "#,
            vec![
                Value::from(chain_id.as_str()),
                Value::Bytes(Some(Box::new(address_bytes))),
                Value::from(blockscout_url.as_str()),
            ],
        );

        let row = txn
            .query_one(insert_contract_stmt)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::internal("failed to insert smart_contracts"))?;
        let contract_id: i64 = row.try_get("", "id")
            .map_err(|e| Status::internal(format!("failed to get inserted id: {e}")))?;

        // Upsert sources
        for (file_name, content) in contract.sources {
            let upsert_source_stmt = Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                INSERT INTO smart_contract_sources (contract_id, file_name, content)
                VALUES ($1, $2, $3)
                ON CONFLICT (contract_id, file_name)
                DO UPDATE SET content = EXCLUDED.content
                "#,
                vec![
                    Value::BigInt(Some(contract_id)),
                    Value::from(file_name.as_str()),
                    Value::from(content.as_str()),
                ],
            );

            txn.execute(upsert_source_stmt)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

        txn.commit().await.map_err(|e| Status::internal(e.to_string()))?;

        let response = SmartContractServiceCreateResponse {};
        Ok(Response::new(response))
    }

    async fn smart_contract_service_get(
        &self,
        request: Request<SmartContractServiceGetRequest>,
    ) -> Result<Response<SmartContractServiceGetResponse>, Status> {
        let req = request.into_inner();

        // Validate and normalize address to 20-byte value
        let addr = Address::from_str(req.address.as_str())
            .map_err(|e| Status::invalid_argument(format!("invalid address: {e}")))?;
        let addr_bytes: Vec<u8> = addr.as_slice().to_vec();

        // Fetch the contract row
        let select_contract_stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT id, chain_id, address, blockscout_url
            FROM smart_contracts
            WHERE chain_id = $1 AND address = $2
            LIMIT 1
            "#,
            vec![
                Value::from(req.chain_id.as_str()),
                Value::Bytes(Some(Box::new(addr_bytes))),
            ],
        );

        let row_opt = self.db
            .query_one(select_contract_stmt)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let Some(row) = row_opt else {
            // Not found
            return Ok(Response::new(SmartContractServiceGetResponse { contract: None }));
        };

        let contract_id: i64 = row.try_get("", "id")
            .map_err(|e| Status::internal(format!("failed to read id: {e}")))?;
        let chain_id: String = row.try_get("", "chain_id")
            .map_err(|e| Status::internal(format!("failed to read chain_id: {e}")))?;
        let address_db: Vec<u8> = row.try_get("", "address")
            .map_err(|e| Status::internal(format!("failed to read address: {e}")))?;
        let blockscout_url: String = row.try_get("", "blockscout_url")
            .map_err(|e| Status::internal(format!("failed to read blockscout_url: {e}")))?;

        // Rebuild Address from DB and format as 0x-hex
        let address_hex = if address_db.len() == 20 {
            let a = Address::from_slice(&address_db);
            format!("{a:#x}")
        } else {
            // Should never happen due to DB constraint, but fallback to request string
            req.address
        };

        // Fetch sources
        let select_sources_stmt = Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT file_name, content
            FROM smart_contract_sources
            WHERE contract_id = $1
            "#,
            vec![Value::BigInt(Some(contract_id))],
        );

        let rows = self.db
            .query_all(select_sources_stmt)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut sources: BTreeMap<String, String> = BTreeMap::new();
        for r in rows {
            let file_name: String = r.try_get("", "file_name")
                .map_err(|e| Status::internal(format!("failed to read file_name: {e}")))?;
            let content: String = r.try_get("", "content")
                .map_err(|e| Status::internal(format!("failed to read content: {e}")))?;
            sources.insert(file_name, content);
        }

        let contract = SmartContract {
            chain_id,
            address: address_hex,
            blockscout_url,
            sources,
        };

        Ok(Response::new(SmartContractServiceGetResponse {
            contract: Some(contract),
        }))
    }
}
