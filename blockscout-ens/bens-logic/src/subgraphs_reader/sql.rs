use super::SubgraphReadError;
use crate::{entity::subgraph::{
    domain::Domain,
    domain_event::{DomainEventTransaction},
}};
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
to_timestamp(expiry_date) < now() AS is_expired 
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


pub async fn find_transaction_events(
    pool: &PgPool,
    schema: &str,
    id: &str,
) -> Result<Vec<DomainEventTransaction>, SubgraphReadError> {
    let sql = sql_events_of_domain(schema);
    let transactions: Vec<DomainEventTransaction> =
        sqlx::query_as(&sql).bind(id).fetch_all(pool).await?;
    Ok(transactions)
}


fn sql_events_of_domain(schema: &str) -> String {
    let domain_events = vec![
        "transfer",
        "new_owner",
        "new_resolver",
        "new_ttl",
        "wrapped_transfer",
        "name_wrapped",
        "name_unwrapped",
        "fuses_set",
        "expiry_extended",
    ]
    .into_iter()
    .map(|table_name| {
        format!(
            r#"
        SELECT '{table_name}' as table_name, block_number, transaction_id
        FROM {schema}.{table_name}
        WHERE domain = $1"#
        )
    });

    let resolver_events = vec![
        "addr_changed",
        "multicoin_addr_changed",
        "name_changed",
        "abi_changed",
        "pubkey_changed",
        "text_changed",
        "contenthash_changed",
        "interface_changed",
        "authorisation_changed",
        "version_changed",
    ]
    .into_iter()
    .map(|table_name| {
        format!(
            r#"
        SELECT '{table_name}' as table_name, t.block_number, t.transaction_id 
        FROM {schema}.{table_name} t
        JOIN {schema}.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1"#
        )
    });

    let registration_events = vec!["name_registered", "name_renewed", "name_transferred"]
        .into_iter()
        .map(|table_name| {
            format!(
                r#"
        SELECT '{table_name}' as table_name, t.block_number, t.transaction_id 
        FROM {schema}.{table_name} t
        JOIN {schema}.registration r
        ON t.registration = r.id
        WHERE r.domain = $1"#
            )
        });
    let events_sql = domain_events
        .chain(resolver_events)
        .chain(registration_events)
        .collect::<Vec<_>>()
        .join("\n        UNION ALL\n");

    let sql = format!(
        r#"
SELECT transaction_id, block_number, array_agg(table_name)
FROM (
    SELECT distinct on (transaction_id, table_name) *
    FROM (
        {events_sql}
    ) all_events
    ORDER BY transaction_id
) unique_events
GROUP BY transaction_id, block_number
ORDER BY block_number
"#
    );

    sql
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn it_works() {
        let sql = sql_events_of_domain("sgd1");
        let expected = r#"
SELECT transaction_id, block_number, array_agg(table_name)
FROM (
    SELECT distinct on (transaction_id, table_name) *
    FROM (
        
        SELECT 'transfer' as table_name, block_number, transaction_id
        FROM sgd1.transfer
        WHERE domain = $1
        UNION ALL

        SELECT 'new_owner' as table_name, block_number, transaction_id
        FROM sgd1.new_owner
        WHERE domain = $1
        UNION ALL

        SELECT 'new_resolver' as table_name, block_number, transaction_id
        FROM sgd1.new_resolver
        WHERE domain = $1
        UNION ALL

        SELECT 'new_ttl' as table_name, block_number, transaction_id
        FROM sgd1.new_ttl
        WHERE domain = $1
        UNION ALL

        SELECT 'wrapped_transfer' as table_name, block_number, transaction_id
        FROM sgd1.wrapped_transfer
        WHERE domain = $1
        UNION ALL

        SELECT 'name_wrapped' as table_name, block_number, transaction_id
        FROM sgd1.name_wrapped
        WHERE domain = $1
        UNION ALL

        SELECT 'name_unwrapped' as table_name, block_number, transaction_id
        FROM sgd1.name_unwrapped
        WHERE domain = $1
        UNION ALL

        SELECT 'fuses_set' as table_name, block_number, transaction_id
        FROM sgd1.fuses_set
        WHERE domain = $1
        UNION ALL

        SELECT 'expiry_extended' as table_name, block_number, transaction_id
        FROM sgd1.expiry_extended
        WHERE domain = $1
        UNION ALL

        SELECT 'addr_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.addr_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'multicoin_addr_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.multicoin_addr_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'name_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'abi_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.abi_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'pubkey_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.pubkey_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'text_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.text_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'contenthash_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.contenthash_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'interface_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.interface_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'authorisation_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.authorisation_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'version_changed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.version_changed t
        JOIN sgd1.resolver r 
        ON t.resolver = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'name_registered' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_registered t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'name_renewed' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_renewed t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
        UNION ALL

        SELECT 'name_transferred' as table_name, t.block_number, t.transaction_id 
        FROM sgd1.name_transferred t
        JOIN sgd1.registration r
        ON t.registration = r.id
        WHERE r.domain = $1
    ) all_events
    ORDER BY transaction_id
) unique_events
GROUP BY transaction_id, block_number
ORDER BY block_number
"#;

        assert_eq!(sql, expected);
    }
}
