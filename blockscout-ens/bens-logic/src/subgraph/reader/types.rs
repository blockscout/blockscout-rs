use crate::{
    entity::subgraph::domain::{DetailedDomain, Domain},
    protocols::{Network, Protocol, ProtocolError},
    subgraph::{sql::DbErr, DomainPaginationInput, Order},
};
use alloy::primitives::Address;
use nonempty::NonEmpty;
use sea_query::{Alias, IntoIden};
use serde::Deserialize;
use std::{fmt::Display, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SubgraphReadError {
    #[error("failed to get protocol info: {0}")]
    Protocol(#[from] ProtocolError),
    #[error("Db err")]
    DbErr(#[from] DbErr),
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl SubgraphReadError {
    /// Logs sqlx pool occupancy and Postgres details when a request fails on the DB path.
    /// Pass the same pool the service uses so `PoolTimedOut` can be correlated with saturation.
    pub fn log_database_diagnostics(&self, pool: Option<&sqlx::PgPool>) {
        let Some(pool) = pool else {
            return;
        };
        let pool_size = pool.size();
        let pool_num_idle = pool.num_idle();
        let pool_max_connections = pool.options().get_max_connections();
        let pool_acquire_timeout_secs = pool.options().get_acquire_timeout().as_secs_f64();

        match self {
            SubgraphReadError::DbErr(DbErr::Sqlx(e)) => {
                if matches!(e, sqlx::Error::PoolTimedOut) {
                    tracing::error!(
                        target: "bens.database",
                        event = "pool_timed_out",
                        pool_size,
                        pool_num_idle,
                        pool_max_connections,
                        pool_acquire_timeout_secs,
                        "sqlx pool acquire timed out (all connections busy or waiting on Postgres); check overlap with refresh_cache, graph-node writers, and DB max_connections"
                    );
                } else if let Some(db) = e.as_database_error() {
                    tracing::error!(
                        target: "bens.database",
                        event = "postgres_error",
                        pool_size,
                        pool_num_idle,
                        pool_max_connections,
                        pg_code = db.code().map(|c| c.to_string()),
                        pg_message = %db.message(),
                        error = %e,
                        error_debug = ?e,
                        "postgres returned an error to sqlx"
                    );
                } else {
                    tracing::error!(
                        target: "bens.database",
                        event = "sqlx_error",
                        pool_size,
                        pool_num_idle,
                        pool_max_connections,
                        error = %e,
                        error_debug = ?e,
                        "sqlx error"
                    );
                }
            }
            SubgraphReadError::DbErr(DbErr::Internal(source)) => {
                tracing::error!(
                    target: "bens.database",
                    event = "db_internal",
                    pool_size,
                    pool_num_idle,
                    pool_max_connections,
                    error = %source,
                    "internal error surfaced as DbErr"
                );
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetDomainInput {
    pub name: String,
    pub network_id: Option<i64>,
    pub protocol_id: Option<String>,
    pub only_active: bool,
}

#[derive(Debug, Clone)]
pub struct GetDomainHistoryInput {
    pub name: String,
    pub network_id: Option<i64>,
    pub protocol_id: Option<String>,
    pub sort: EventSort,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct LookupDomainInput {
    pub name: Option<String>,
    pub only_active: bool,
    pub network_id: Option<i64>,
    pub protocols: Option<NonEmpty<String>>,
    pub pagination: DomainPaginationInput,
}

#[derive(Debug, Clone)]
pub struct LookupAddressInput {
    pub address: Address,
    pub resolved_to: bool,
    pub owned_by: bool,
    pub only_active: bool,
    pub network_id: Option<i64>,
    pub protocols: Option<NonEmpty<String>>,
    pub pagination: DomainPaginationInput,
}

#[derive(Debug, Clone)]
pub struct GetAddressInput {
    pub address: Address,
    pub network_id: Option<i64>,
    pub protocols: Option<NonEmpty<String>>,
}

impl Default for DomainPaginationInput {
    fn default() -> Self {
        Self {
            sort: Default::default(),
            order: Default::default(),
            page_size: 50,
            page_token: Default::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BatchResolveAddressNamesInput {
    pub addresses: Vec<Address>,
    pub network_id: Option<i64>,
    pub protocols: Option<NonEmpty<String>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum DomainSortField {
    #[default]
    RegistrationDate,
}

impl DomainSortField {
    pub fn to_database_field(&self) -> sea_query::ColumnRef {
        let col = match self {
            DomainSortField::RegistrationDate => "created_at",
        };
        sea_query::ColumnRef::Column(Alias::new(col).into_iden())
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum EventSort {
    #[default]
    BlockNumber,
}

impl Display for EventSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventSort::BlockNumber => write!(f, "block_number"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GetDomainOutput {
    pub domain: DetailedDomain,
    pub tokens: Vec<DomainToken>,
    pub protocol: Protocol,
    pub deployment_network: Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainToken {
    pub id: String,
    pub contract: Address,
    pub _type: DomainTokenType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainTokenType {
    Native,
    Wrapped,
}

#[derive(Debug, Clone)]
pub struct LookupOutput {
    pub domain: Domain,
    pub protocol: Protocol,
    pub deployment_network: Network,
}

#[derive(Debug, Clone)]
pub struct ResolverInSubgraph {
    pub resolver_address: Address,
    pub domain_id: String,
}

impl FromStr for ResolverInSubgraph {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (resolver_address, domain_id) = value
            .split_once('-')
            .ok_or_else(|| anyhow::anyhow!("Invalid resolver in subgraph format: {}", value))?;
        let resolver_address = Address::from_str(resolver_address)?;

        Ok(Self {
            domain_id: domain_id.to_string(),
            resolver_address,
        })
    }
}

impl Display for ResolverInSubgraph {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.resolver_address, self.domain_id)
    }
}

impl ResolverInSubgraph {
    pub fn new(resolver_address: Address, domain_id: String) -> Self {
        Self {
            resolver_address,
            domain_id,
        }
    }
}
