use ethers::types::Address;
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
    pub name: String,
    pub only_active: bool,
    pub sort: DomainSort,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct LookupAddressInput {
    pub network_id: i64,
    pub address: Address,
    pub resolved_to: bool,
    pub owned_by: bool,
    pub only_active: bool,
    pub sort: DomainSort,
    pub order: Order,
}

#[derive(Debug, Clone)]
pub struct BatchResolveAddressNamesInput {
    pub network_id: i64,
    pub addresses: Vec<Address>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum DomainSort {
    #[default]
    RegistrationDate,
}

impl Display for DomainSort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DomainSort::RegistrationDate => write!(f, "registration_date"),
        }
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

#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum Order {
    #[default]
    Asc,
    Desc,
}

impl Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Order::Asc => write!(f, "asc"),
            Order::Desc => write!(f, "desc"),
        }
    }
}
