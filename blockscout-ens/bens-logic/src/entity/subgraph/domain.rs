use chrono::Utc;
use sqlx::types::BigDecimal;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct DetailedDomain {
    pub vid: i64,
    pub id: String,
    pub name: Option<String>,
    pub label_name: Option<String>,
    pub labelhash: Option<Vec<u8>>,
    pub parent: Option<String>,
    pub subdomain_count: i32,
    pub resolved_address: Option<String>,
    pub resolver: Option<String>,
    pub ttl: Option<chrono::DateTime<Utc>>,
    pub is_migrated: bool,
    pub registration_date: chrono::DateTime<Utc>,
    pub owner: String,
    pub registrant: Option<String>,
    pub wrapped_owner: Option<String>,
    pub created_at: BigDecimal,
    pub expiry_date: Option<chrono::DateTime<Utc>>,
    pub is_expired: bool,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
    pub protocol_slug: String,
    #[sqlx(default)]
    pub other_addresses: sqlx::types::Json<HashMap<String, String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct Domain {
    pub vid: i64,
    pub id: String,
    pub name: Option<String>,
    pub resolved_address: Option<String>,
    pub resolver: Option<String>,
    pub registration_date: chrono::DateTime<Utc>,
    pub owner: String,
    pub wrapped_owner: Option<String>,
    pub created_at: BigDecimal,
    pub expiry_date: Option<chrono::DateTime<Utc>>,
    pub is_expired: bool,
    pub protocol_slug: String,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CreationDomain {
    pub vid: Option<i64>,
    pub id: String,
    pub name: Option<String>,
    pub resolved_address: Option<String>,
    pub resolver: Option<String>,
    pub owner: String,
    pub wrapped_owner: Option<String>,
    pub created_at: BigDecimal,
    pub expiry_date: Option<BigDecimal>,
    pub label_name: Option<String>,
    pub labelhash: Option<Vec<u8>>,
    pub parent: Option<String>,
    pub subdomain_count: i32,
    pub is_expired: bool,
    pub is_migrated: bool,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct CreationAddr2Name {
    pub resolved_address: String,
    pub domain_id: Option<String>,
    pub domain_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DomainWithAddress {
    pub id: String,
    pub domain_name: String,
    pub resolved_address: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ReverseRecord {
    pub addr_reverse_id: String,
    pub reversed_name: String,
    pub protocol_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct AddrReverseDomainWithActualName {
    pub domain_id: String,
    pub reversed_domain_id: String,
    pub resolved_address: String,
    pub name: String,
}

impl From<DetailedDomain> for Domain {
    fn from(domain: DetailedDomain) -> Self {
        Self {
            vid: domain.vid,
            id: domain.id,
            name: domain.name,
            resolved_address: domain.resolved_address,
            resolver: domain.resolver,
            registration_date: domain.registration_date,
            owner: domain.owner,
            wrapped_owner: domain.wrapped_owner,
            created_at: domain.created_at,
            expiry_date: domain.expiry_date,
            is_expired: domain.is_expired,
            protocol_slug: domain.protocol_slug,
            stored_offchain: domain.stored_offchain,
            resolved_with_wildcard: domain.resolved_with_wildcard,
        }
    }
}
