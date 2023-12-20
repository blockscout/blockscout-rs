use crate::{
    entity::subgraph::domain::{DetailedDomain, Domain, DomainWithAddress},
    hash_name::hex,
    subgraphs_reader::{GetDomainInput, LookupAddressInput, LookupDomainInput, SubgraphReadError},
};
use sqlx::postgres::{PgPool, PgQueryResult};
use tracing::instrument;

const DETAILED_DOMAIN_DEFAULT_SELECT_CLAUSE: &str = r#"
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
to_timestamp(created_at) as registration_date,
owner,
registrant,
wrapped_owner,
to_timestamp(expiry_date) as expiry_date,
COALESCE(to_timestamp(expiry_date) < now(), false) AS is_expired
"#;

const DOMAIN_DEFAULT_SELECT_CLAUSE: &str = r#"
id,
name,
resolved_address,
to_timestamp(created_at) as registration_date,
owner,
to_timestamp(expiry_date) as expiry_date,
COALESCE(to_timestamp(expiry_date) < now(), false) AS is_expired
"#;

// `block_range @>` is special sql syntax for fast filtering int4range
// to access current version of domain.
// Source: https://github.com/graphprotocol/graph-node/blob/19fd41bb48511f889dc94f5d82e16cd492f29da1/store/postgres/src/block_range.rs#L26
pub const DOMAIN_BLOCK_RANGE_WHERE_CLAUSE: &str = "block_range @> 2147483647";

pub const DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE: &str = "label_name IS NOT NULL";

pub const DOMAIN_NOT_EXPIRED_WHERE_CLAUSE: &str = r#"
(
    expiry_date is null
    OR to_timestamp(expiry_date) > now()
)
"#;

#[instrument(name = "get_domain", skip(pool), err(level = "error"), level = "info")]
pub async fn get_domain(
    pool: &PgPool,
    id: &str,
    schema: &str,
    input: &GetDomainInput,
) -> Result<Option<DetailedDomain>, SubgraphReadError> {
    let only_active_clause = input
        .only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT
            {DETAILED_DOMAIN_DEFAULT_SELECT_CLAUSE},
            COALESCE(
                multi_coin_addresses.coin_to_addr,
                '{{}}'::json
            )as other_addresses
        FROM {schema}.domain
        LEFT JOIN (
            SELECT 
                d.id as domain_id, json_object_agg(mac.coin_type, encode(mac.addr, 'hex')) AS coin_to_addr 
            FROM {schema}.domain d
            LEFT JOIN {schema}.multicoin_addr_changed mac ON d.resolver = mac.resolver
            WHERE 
                d.id = $1
                AND d.block_range @> 2147483647
                AND mac.coin_type IS NOT NULL
                AND mac.addr IS NOT NULL
            GROUP BY d.id
        ) multi_coin_addresses ON {schema}.domain.id = multi_coin_addresses.domain_id
        WHERE 
            id = $1 
            AND {DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
        {only_active_clause}
        ;"#,
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

#[instrument(
    name = "find_domains",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn find_domains(
    pool: &PgPool,
    schema: &str,
    id: &str,
    input: &LookupDomainInput,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let only_active_clause = input
        .only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let sort = input.sort;
    let order = input.order;
    let domains = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE
            id = $1 
            AND {DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
            {only_active_clause}
        ORDER BY {sort} {order}
        "#,
    ))
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(domains)
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
    input: &LookupAddressInput,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let only_active_clause = input
        .only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let address = hex(input.address);
    let resolved_to_clause = input
        .resolved_to
        .then_some("OR resolved_address = $1")
        .unwrap_or_default();
    let owned_by_clause = input
        .owned_by
        .then_some("OR owner = $1 OR wrapped_owner = $1")
        .unwrap_or_default();
    let sort = input.sort;
    let order = input.order;

    let resolved_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE 
            (
                FALSE
                {resolved_to_clause}
                {owned_by_clause}
            )
            AND {DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
            AND {DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE}
            {only_active_clause}
        ORDER BY {sort} {order}
        LIMIT 100
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(resolved_domains)
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
            AND {DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
            AND {DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE}
            AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}
        ORDER BY resolved_address, created_at
        "#,
    ))
    .bind(addresses)
    .fetch_all(pool)
    .await?;

    Ok(domains)
}

#[instrument(
    name = "batch_search_addresses_cached",
    skip(pool, addresses),
    fields(job_size = addresses.len()),
    err(level = "error"),
    level = "info",
)]
pub async fn batch_search_addresses_cached(
    pool: &PgPool,
    schema: &str,
    addresses: &[&str],
) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
    let domains: Vec<DomainWithAddress> = sqlx::query_as(&format!(
        r#"
        SELECT id, domain_name, resolved_address
        FROM {schema}.address_names
        where
            resolved_address = ANY($1)
        "#,
    ))
    .bind(addresses)
    .fetch_all(pool)
    .await?;

    Ok(domains)
}

#[instrument(
    name = "update_domain_name",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn update_domain_name(
    pool: &PgPool,
    schema: &str,
    id: &str,
    name: &str,
) -> Result<PgQueryResult, sqlx::Error> {
    let result = sqlx::query(&format!(
        "UPDATE {schema}.domain SET name = $1 WHERE id = $2;"
    ))
    .bind(name)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(result)
}
