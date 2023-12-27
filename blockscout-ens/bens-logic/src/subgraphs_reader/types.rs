use super::pagination::{DomainPaginationInput, Order};
use ethers::types::Address;
use sea_query::{Alias, IntoIden};
use serde::Deserialize;
use std::fmt::Display;

#[derive(Debug, Clone)]
pub struct GetDomainInput {
    pub network_id: i64,
    pub name: String,
    pub only_active: bool,
}

#[derive(Debug, Clone)]
pub struct GetDomainHistoryInput {
    pub network_id: i64,
    pub name: String,
    pub sort: EventSort,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct LookupDomainInput {
    pub network_id: i64,
    pub name: Option<String>,
    pub only_active: bool,
    pub pagination: DomainPaginationInput,
}

#[derive(Debug, Clone)]
pub struct LookupAddressInput {
    pub network_id: i64,
    pub address: Address,
    pub resolved_to: bool,
    pub owned_by: bool,
    pub only_active: bool,
    pub pagination: DomainPaginationInput,
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
    pub network_id: i64,
    pub addresses: Vec<Address>,
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
