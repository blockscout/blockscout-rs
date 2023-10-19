use super::schema_selector::schema_names;
use crate::{entity::subgraph::domain::Domain, hash_name::hash_ens_domain_name};
use sqlx::postgres::PgPool;
use std::{collections::HashMap, sync::Arc};
use thiserror::Error;

pub struct SubgraphReader {
    pool: Arc<PgPool>,
    schema_names: HashMap<i64, String>,
}

impl SubgraphReader {
    pub async fn initialize(pool: Arc<PgPool>) -> Result<Self, anyhow::Error> {
        let schema_names = schema_names(&pool).await?;
        Ok(Self::new(pool, schema_names))
    }

    pub fn new(pool: Arc<PgPool>, schema_names: HashMap<i64, String>) -> Self {
        Self { pool, schema_names }
    }
}

#[derive(Error, Debug)]
pub enum SubgraphReadError {
    #[error("Network with id {0} not found")]
    NetworkNotFound(i64),
    #[error("Db err")]
    DbErr(#[from] sqlx::Error),
}

impl SubgraphReader {
    pub async fn get_domain(
        &self,
        network_id: i64,
        name: &str,
    ) -> Result<Option<Domain>, SubgraphReadError> {
        let schema = self
            .schema_names
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let id = domain_id(name);
        find_domain(self.pool.as_ref(), schema, &id).await
    }

    pub async fn get_domain_history(
        &self,
        _network_id: i64,
        _name: &str,
    ) -> Result<Vec<()>, SubgraphReadError> {
        todo!()
    }

    pub async fn search_resolved_domain_reverse(
        &self,
        network_id: i64,
        address: ethers::types::Address,
    ) -> Result<Vec<Domain>, SubgraphReadError> {
        let schema = self
            .schema_names
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let address = hex(address);
        find_resolved_addresses(self.pool.as_ref(), schema, &address).await
    }

    pub async fn search_owned_domain_reverse(
        &self,
        network_id: i64,
        address: ethers::types::Address,
    ) -> Result<Vec<Domain>, SubgraphReadError> {
        let schema = self
            .schema_names
            .get(&network_id)
            .ok_or_else(|| SubgraphReadError::NetworkNotFound(network_id))?;
        let address = hex(address);
        find_owned_addresses(self.pool.as_ref(), schema, &address).await
    }
}

async fn find_domain(
    pool: &PgPool,
    schema: &str,
    id: &str,
) -> Result<Option<Domain>, SubgraphReadError> {
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT
        DISTINCT ON (id) *
        FROM {schema}.domain
        WHERE
            id = $1 
            AND name IS NOT NULL
            AND to_timestamp(expiry_date) > now()
        ORDER BY
            id,
            block_range DESC
        "#,
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

async fn find_resolved_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let resolved_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT * FROM (
            SELECT 
            DISTINCT ON (id) *
            FROM {schema}.domain
            WHERE 
                resolved_address = $1
                AND name IS NOT NULL 
                AND to_timestamp(expiry_date) > now()
            ORDER BY
                id,
                block_range DESC
        ) sub
        ORDER BY created_at ASC
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(resolved_domains)
}

async fn find_owned_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let owned_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT * FROM (
            SELECT 
            DISTINCT ON (id) *
            FROM {schema}.domain
            WHERE 
                (
                    owner = $1
                    OR wrapped_owner = $1
                )
                AND name IS NOT NULL
                AND to_timestamp(expiry_date) > now()
            ORDER BY
                id,
                block_range DESC
        ) sub
        ORDER BY created_at ASC
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(owned_domains)
}

fn domain_id(name: &str) -> String {
    hex(hash_ens_domain_name(name))
}

fn hex<T>(data: T) -> String
where
    T: AsRef<[u8]>,
{
    format!("0x{}", hex::encode(data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[sqlx::test(migrations = "tests/migrations")]
    async fn get_domain_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let reader = SubgraphReader::initialize(pool.clone())
            .await
            .expect("failed to init reader");

        // get vitalik domain
        let result = reader
            .get_domain(1, "vitalik.eth")
            .await
            .expect("failed to get vitalik domain")
            .expect("domain not found");
        assert_eq!(result.name.as_deref(), Some("vitalik.eth"));
        assert_eq!(
            result.resolved_address.as_deref(),
            Some("0xd8da6bf26964af9d7eed9e03e53415d37aa96045")
        );
        // get expired domain
        let result = reader
            .get_domain(1, "expired.eth")
            .await
            .expect("failed to get expired domain");
        assert!(result.is_none(), "expired domain returned: {:?}", result);
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn search_domain_reverse_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let reader = SubgraphReader::initialize(pool.clone())
            .await
            .expect("failed to init reader");

        let result = reader
            .search_resolved_domain_reverse(1, addr("d8da6bf26964af9d7eed9e03e53415d37aa96045"))
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth"), Some("sashaxyz.eth")]
        );

        let result = reader
            .search_owned_domain_reverse(1, addr("d8da6bf26964af9d7eed9e03e53415d37aa96045"))
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth")]
        );
    }

    fn addr(a: &str) -> ethers::types::Address {
        ethers::types::Address::from_slice(hex::decode(a).unwrap().as_slice())
    }
}
