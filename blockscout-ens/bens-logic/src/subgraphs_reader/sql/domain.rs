use crate::{entity::subgraph::domain::Domain, subgraphs_reader::SubgraphReadError};
use sqlx::postgres::PgPool;

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
name IS NOT NULL
AND block_range @> 2147483647
"#;

const DOMAIN_NOT_EXPIRED_WHERE_CLAUSE: &str = r#"
(
    expiry_date is null
    OR to_timestamp(expiry_date) > now()
)
"#;

pub async fn find_domain(
    pool: &PgPool,
    schema: &str,
    id: &str,
) -> Result<Option<Domain>, SubgraphReadError> {
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE
            id = $1 
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
        "#,
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

pub async fn find_resolved_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
) -> Result<Vec<Domain>, SubgraphReadError> {
    let resolved_domains: Vec<Domain> = sqlx::query_as(&format!(
        r#"
        SELECT {DOMAIN_DEFAULT_SELECT_CLAUSE}
        FROM {schema}.domain
        WHERE 
            resolved_address = $1
            AND {DOMAIN_DEFAULT_WHERE_CLAUSE}
            AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}
        ORDER BY created_at ASC
        LIMIT 100
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(resolved_domains)
}

pub async fn find_owned_addresses(
    pool: &PgPool,
    schema: &str,
    address: &str,
) -> Result<Vec<Domain>, SubgraphReadError> {
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
            AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}
        ORDER BY created_at ASC
        LIMIT 100
        "#,
    ))
    .bind(address)
    .fetch_all(pool)
    .await?;

    Ok(owned_domains)
}
