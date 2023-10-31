use crate::{
    entity::subgraph::domain::{Domain, DomainWithAddress},
    subgraphs_reader::SubgraphReadError,
};
use sqlx::postgres::PgPool;
use tracing::instrument;

const DOMAIN_DEFAULT_SELECT_CLAUSE: &str = r#"
vid,
block_range,
id,
name,
label_name,
labelhash,
parent,
subdomain_count,
resolved_address,
resolver,
to_timestamp(ttl) as ttl,
is_migrated,
to_timestamp(created_at) as created_at,
owner,
registrant,
wrapped_owner,
to_timestamp(expiry_date) as expiry_date,
COALESCE(to_timestamp(expiry_date) < now(), false) AS is_expired 
"#;

// `block_range @>` is special sql syntax for fast filtering int4range
// to access current version of domain.
// Source: https://github.com/graphprotocol/graph-node/blob/19fd41bb48511f889dc94f5d82e16cd492f29da1/store/postgres/src/block_range.rs#L26
const DOMAIN_DEFAULT_WHERE_CLAUSE: &str = r#"
label_name IS NOT NULL
AND block_range @> 2147483647
"#;

const DOMAIN_NOT_EXPIRED_WHERE_CLAUSE: &str = r#"
(
    expiry_date is null
    OR to_timestamp(expiry_date) > now()
)
"#;

#[instrument(name = "find_domain", skip(pool), err(level = "error"), level = "info")]
pub async fn find_domain(
    pool: &PgPool,
    schema: &str,
    id: &str,
    only_active: bool,
) -> Result<Option<Domain>, SubgraphReadError> {
    let only_active_clause = only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE
            id = $1 
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
            {only_active_clause}
        "#,
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

#[instrument(
    name = "find_resolved_addresses",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn find_resolved_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
    only_active: bool,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let only_active_clause = only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let resolved_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE 
            resolved_address = $1
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
            {only_active_clause}
        ORDER BY created_at ASC
        LIMIT 100
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(resolved_domains)
}

#[instrument(
    name = "find_owned_addresses",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn find_owned_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
    only_active: bool,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let only_active_clause = only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let owned_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE 
            (
                owner = $1
                OR wrapped_owner = $1
            )
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
            {only_active_clause}
        ORDER BY created_at ASC
        LIMIT 100
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(owned_domains)
}

#[instrument(
    name = "batch_search_addresses",
    skip(pool, addresses),
    fields(job_size = addresses.len()),
    err(level = "error"),
    level = "info",
)]
pub async fn batch_search_addresses(
    pool: &PgPool,
    schema: &str,
    addresses: &[&str],
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let domains: Vec<DomainWithAddress> = sqlx::query_as(&format!(
        r#"
        SELECT DISTINCT ON (resolved_address) id, name AS domain_name, resolved_address 
        FROM {schema}.domain
        WHERE
            resolved_address = ANY($1)
            AND name NOT LIKE '%[%'
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
            AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}
        ORDER BY resolved_address, created_at
        "#,
    ))
    .bind(addresses)
    .fetch_all(pool)
    .await?;

    Ok(domains)
}
