use super::{schema_selector::schema_names, sql};
use crate::{entity::subgraph::{domain::Domain, domain_event::DomainEvent}, hash_name::hash_ens_domain_name};
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
        sql::find_domain(self.pool.as_ref(), schema, &id).await
    }

    pub async fn get_domain_history(
        &self,
        _network_id: i64,
        _name: &str,
    ) -> Result<Vec<DomainEvent>, SubgraphReadError> {
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
        sql::find_resolved_addresses(self.pool.as_ref(), schema, &address).await
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
        sql::find_owned_addresses(self.pool.as_ref(), schema, &address).await
    }
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
            .expect("failed to get expired domain")
            .expect("expired domain not found");
        assert!(
            result.is_expired,
            "expired domain has is_expired=false: {:?}",
            result
        );
    }

    #[sqlx::test(migrations = "tests/migrations")]
    async fn search_domain_reverse_works(pool: PgPool) {
        let pool = Arc::new(pool);
        let reader = SubgraphReader::initialize(pool.clone())
            .await
            .expect("failed to init reader");

        let result = reader
            .search_resolved_domain_reverse(1, addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"))
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth"), Some("sashaxyz.eth")]
        );

        let result = reader
            .search_owned_domain_reverse(1, addr("0xd8da6bf26964af9d7eed9e03e53415d37aa96045"))
            .await
            .expect("failed to get vitalik domains");
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![Some("vitalik.eth")]
        );

        // search for expired address
        let result = reader
            .search_resolved_domain_reverse(1, addr("0x9f7f7ddbfb8e14d1756580ba8037530da0880b99"))
            .await
            .expect("failed to get expired domains");
        // expired domain shoudn't be returned as resolved
        assert_eq!(
            result.iter().map(|d| d.name.as_deref()).collect::<Vec<_>>(),
            vec![]
        );
    }

    fn addr(a: &str) -> ethers::types::Address {
        let a = a.trim_start_matches("0x");
        ethers::types::Address::from_slice(
            hex::decode(a)
                .expect("invalid hex provided in addr()")
                .as_slice(),
        )
    }
}
