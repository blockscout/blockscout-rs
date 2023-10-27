use chrono::Utc;

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Domain {
    pub vid: i64,
    pub block_range: sqlx::postgres::types::PgRange<i32>,
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
    pub created_at: chrono::DateTime<Utc>,
    pub owner: String,
    pub registrant: Option<String>,
    pub wrapped_owner: Option<String>,
    pub expiry_date: Option<chrono::DateTime<Utc>>,
    pub is_expired: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct DomainWithAddress {
    pub id: String,
    pub domain_name: String,
    pub resolved_address: String,
}
