use chrono::Utc;
use sqlx::types::BigDecimal;
use std::collections::HashMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct DetailedDomain {
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
    pub expiry_date: Option<chrono::DateTime<Utc>>,
    pub is_expired: bool,
    pub stored_offchain: bool,
    pub resolved_with_wildcard: bool,
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
