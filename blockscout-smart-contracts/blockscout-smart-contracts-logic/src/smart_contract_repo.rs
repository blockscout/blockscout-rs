use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, Statement, TransactionTrait};
use sea_orm::sea_query::Value;
use std::collections::BTreeMap;
use std::sync::Arc;
use tonic::Status;

use crate::create_input::CreateInput;
use crate::records::ContractRecord;

pub async fn upsert_contract(
    connection: &Arc<DatabaseConnection>,
    input: &CreateInput,
) -> Result<(), Status> {
    let txn = connection.begin().await.map_err(|e| Status::internal(e.to_string()))?;
    let contract_id = upsert_contract_returning_id(&txn, &input).await?;
    upsert_sources(&txn, contract_id, &input.sources).await?;
    txn.commit().await.map_err(|e| Status::internal(e.to_string()))?;
    Ok(())
}

pub async fn upsert_contract_returning_id(
    exec: &impl ConnectionTrait,
    input: &CreateInput,
) -> Result<i64, Status> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
        INSERT INTO smart_contracts (chain_id, address, blockscout_url)
        VALUES ($1, $2, $3)
        ON CONFLICT (chain_id, address)
        DO UPDATE SET blockscout_url = EXCLUDED.blockscout_url
        RETURNING id
        "#,
        vec![
            Value::from(input.chain_id.as_str()),
            Value::Bytes(Some(Box::new(input.address_bytes.clone()))),
            Value::from(input.blockscout_url.as_str()),
        ],
    );

    let row = exec
        .query_one(stmt)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .ok_or_else(|| Status::internal("failed to insert smart_contracts"))?;

    row.try_get("", "id")
        .map_err(|e| Status::internal(format!("failed to get inserted id: {e}")))
}

pub async fn upsert_sources(
    exec: &impl ConnectionTrait,
    contract_id: i64,
    sources: &BTreeMap<String, String>,
) -> Result<(), Status> {
    for (file_name, content) in sources {
        let stmt = Statement::from_sql_and_values(
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

        exec.execute(stmt)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
    }
    Ok(())
}

pub async fn select_contract(
    exec: &impl ConnectionTrait,
    chain_id: &str,
    address_bytes: Vec<u8>,
) -> Result<Option<ContractRecord>, Status> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
        SELECT id, chain_id, address, blockscout_url
        FROM smart_contracts
        WHERE chain_id = $1 AND address = $2
        LIMIT 1
        "#,
        vec![
            Value::from(chain_id),
            Value::Bytes(Some(Box::new(address_bytes))),
        ],
    );

    let row_opt = exec
        .query_one(stmt)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let Some(row) = row_opt else {
        return Ok(None);
    };

    let id: i64 = row
        .try_get("", "id")
        .map_err(|e| Status::internal(format!("failed to read id: {e}")))?;
    let chain_id: String = row
        .try_get("", "chain_id")
        .map_err(|e| Status::internal(format!("failed to read chain_id: {e}")))?;
    let address_db: Vec<u8> = row
        .try_get("", "address")
        .map_err(|e| Status::internal(format!("failed to read address: {e}")))?;
    let blockscout_url: String = row
        .try_get("", "blockscout_url")
        .map_err(|e| Status::internal(format!("failed to read blockscout_url: {e}")))?;

    Ok(Some(ContractRecord {
        id,
        chain_id,
        address_db,
        blockscout_url,
    }))
}

pub async fn select_sources(
    exec: &impl ConnectionTrait,
    contract_id: i64,
) -> Result<BTreeMap<String, String>, Status> {
    let stmt = Statement::from_sql_and_values(
        DatabaseBackend::Postgres,
        r#"
        SELECT file_name, content
        FROM smart_contract_sources
        WHERE contract_id = $1
        "#,
        vec![Value::BigInt(Some(contract_id))],
    );

    let rows = exec
        .query_all(stmt)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;

    let mut sources = BTreeMap::new();
    for r in rows {
        let file_name: String = r
            .try_get("", "file_name")
            .map_err(|e| Status::internal(format!("failed to read file_name: {e}")))?;
        let content: String = r
            .try_get("", "content")
            .map_err(|e| Status::internal(format!("failed to read content: {e}")))?;
        sources.insert(file_name, content);
    }

    Ok(sources)
}
