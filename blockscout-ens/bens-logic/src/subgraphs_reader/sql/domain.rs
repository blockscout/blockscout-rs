use crate::{
    entity::subgraph::domain::{DetailedDomain, Domain},
    protocols::{hash_name::hex, DomainName, DomainNameOnProtocol, Protocol},
    subgraphs_reader::{
        sql::{utils, DbErr},
        DomainPaginationInput, GetDomainInput, LookupAddressInput,
    },
};
use ethers::addressbook::Address;
use nonempty::NonEmpty;
use sea_query::{Alias, Condition, Expr, PostgresQueryBuilder, SelectStatement};
use sql_gen::QueryBuilderExt;
use sqlx::postgres::{PgPool, PgQueryResult};
use std::collections::HashMap;
use tracing::instrument;

mod sql_gen {
    use super::*;

    pub trait QueryBuilderExt {
        fn with_block_range(&mut self) -> &mut Self;

        fn with_non_empty_label(&mut self) -> &mut Self;

        fn with_not_expired(&mut self) -> &mut Self;

        fn with_resolved_names(&mut self) -> &mut Self;
    }

    impl QueryBuilderExt for sea_query::SelectStatement {
        fn with_block_range(&mut self) -> &mut SelectStatement {
            self.and_where(Expr::cust(DOMAIN_BLOCK_RANGE_WHERE_CLAUSE))
        }

        fn with_non_empty_label(&mut self) -> &mut SelectStatement {
            self.and_where(Expr::cust(DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE))
        }

        fn with_not_expired(&mut self) -> &mut SelectStatement {
            self.and_where(Expr::cust(DOMAIN_NOT_EXPIRED_WHERE_CLAUSE))
        }

        fn with_resolved_names(&mut self) -> &mut SelectStatement {
            self.and_where(Expr::cust("name NOT LIKE '%[%'"))
        }
    }

    #[allow(dead_code)]
    pub fn detailed_domain_select(protocol: &Protocol) -> SelectStatement {
        domain_select_custom(protocol, DETAILED_DOMAIN_DEFAULT_SELECT_CLAUSE)
    }

    pub fn domain_select(protocol: &Protocol) -> SelectStatement {
        domain_select_custom(protocol, DOMAIN_DEFAULT_SELECT_CLAUSE)
    }

    pub fn domain_select_custom(protocol: &Protocol, select: &str) -> SelectStatement {
        sea_query::Query::select()
            .expr(Expr::cust(select))
            .expr_as(
                Expr::cust(format!("'{}'", protocol.info.slug)),
                Alias::new("protocol_slug"),
            )
            .from((Alias::new(&protocol.subgraph_schema), Alias::new("domain")))
            .to_owned()
    }
}

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
created_at,
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
created_at,
to_timestamp(created_at) as registration_date,
owner,
wrapped_owner,
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

// TODO: rewrite to sea_query generation
#[instrument(name = "get_domain", skip(pool), err(level = "error"), level = "info")]
pub async fn get_domain(
    pool: &PgPool,
    domain_name: &DomainNameOnProtocol<'_>,
    input: &GetDomainInput,
) -> Result<Option<DetailedDomain>, DbErr> {
    let only_active_clause = input
        .only_active
        .then(|| format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}"))
        .unwrap_or_default();
    let schema = &domain_name.protocol.subgraph_schema;
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT
            {DETAILED_DOMAIN_DEFAULT_SELECT_CLAUSE},
            '{schema}' as schema_name,
            COALESCE(
                multi_coin_addresses.coin_to_addr,
                '{{}}'::json
            ) as other_addresses
        FROM {schema}.domain
        LEFT JOIN (
            SELECT 
                d.id as domain_id, json_object_agg(mac.coin_type, encode(mac.addr, 'hex')) AS coin_to_addr 
            FROM {schema}.domain d
            LEFT JOIN {schema}.multicoin_addr_changed mac ON d.resolver = mac.resolver
            WHERE 
                d.id = $1
                AND d.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
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
    .bind(&domain_name.inner.id)
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

#[derive(Clone, Debug)]
pub enum FindDomainsInput<'a> {
    Names(Vec<DomainNameOnProtocol<'a>>),
    Protocols(Vec<&'a Protocol>),
}

#[instrument(name = "find_domains", skip_all, err(level = "error"), level = "info")]
pub async fn find_domains(
    pool: &PgPool,
    input: FindDomainsInput<'_>,
    only_active: bool,
    pagination: Option<&DomainPaginationInput>,
) -> Result<Vec<Domain>, DbErr> {
    let queries = match &input {
        FindDomainsInput::Names(names) => {
            let unique_protocols = names
                .iter()
                .map(|name| (name.protocol.subgraph_schema.clone(), name.protocol))
                .collect::<HashMap<_, _>>();
            unique_protocols
                .values()
                .map(|protocol| {
                    let mut query = sql_gen::domain_select(protocol);
                    query.and_where(Expr::cust("id = ANY($1)")).to_owned()
                })
                .collect::<Vec<_>>()
        }
        FindDomainsInput::Protocols(protocols) => protocols
            .iter()
            .map(|protocol| {
                let mut query = sql_gen::domain_select(protocol);
                query
                    .with_non_empty_label()
                    .with_resolved_names()
                    .to_owned()
            })
            .collect::<Vec<_>>(),
    };
    let queries = queries.into_iter().map(|mut q| {
        let mut q = q.with_block_range().to_owned();
        if only_active {
            q.with_not_expired().to_owned()
        } else {
            q
        }
    });

    let sql = match NonEmpty::collect(queries) {
        Some(protocol_queries) => utils::union_domain_queries(protocol_queries, None, pagination)?
            .to_string(PostgresQueryBuilder),
        None => return Ok(Vec::new()),
    };

    let mut query = sqlx::query_as(&sql);
    tracing::debug!(sql = sql, "build SQL query for 'find_domains'");
    if let FindDomainsInput::Names(names) = &input {
        query = query.bind(names.iter().map(|n| n.inner.id.clone()).collect::<Vec<_>>());
    }
    let domains = query.fetch_all(pool).await?;
    Ok(domains)
}

#[instrument(
    name = "find_resolved_addresses",
    skip_all,
    err(level = "error"),
    level = "info"
)]
pub async fn find_resolved_addresses(
    pool: &PgPool,
    protocols: NonEmpty<&Protocol>,
    input: &LookupAddressInput,
) -> Result<Vec<Domain>, DbErr> {
    let queries = protocols.into_iter().map(|protocol| {
        gen_sql_select_domains_by_address(
            protocol,
            None,
            input.only_active,
            input.resolved_to,
            input.owned_by,
        )
    });
    let queries = NonEmpty::collect(queries).expect("protocols are not empty");
    let sql = utils::union_domain_queries(queries, None, Some(&input.pagination))?
        .to_string(PostgresQueryBuilder);
    let domains = sqlx::query_as(&sql)
        .bind(hex(input.address))
        .fetch_all(pool)
        .await?;
    Ok(domains)
}

#[instrument(
    name = "count_domains_by_address",
    skip_all,
    err(level = "error"),
    level = "info"
)]
pub async fn count_domains_by_address(
    pool: &PgPool,
    protocols: NonEmpty<&Protocol>,
    address: Address,
    only_active: bool,
    resolved_to: bool,
    owned_by: bool,
) -> Result<i64, DbErr> {
    let queries = protocols.into_iter().map(|protocol| {
        gen_sql_select_domains_by_address(
            protocol,
            // for counting we don't need to any fields
            Some("1"),
            only_active,
            resolved_to,
            owned_by,
        )
    });
    let queries = NonEmpty::collect(queries).expect("protocols are not empty");
    let sql = utils::union_domain_queries(queries, Some("COUNT(*)"), None)?
        .to_string(PostgresQueryBuilder);

    let count: i64 = sqlx::query_scalar(&sql)
        .bind(hex(address))
        .fetch_one(pool)
        .await?;
    Ok(count)
}

// TODO: rewrite to sea_query generation
#[instrument(
    name = "update_domain_name",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn update_domain_name(
    pool: &PgPool,
    schema: &str,
    name: DomainName,
) -> Result<PgQueryResult, DbErr> {
    let result = sqlx::query(&format!(
        "UPDATE {schema}.domain SET name = $1, label_name = $2 WHERE id = $3;"
    ))
    .bind(&name.name)
    .bind(&name.label_name)
    .bind(&name.id)
    .execute(pool)
    .await?;
    Ok(result)
}

fn gen_sql_select_domains_by_address(
    protocol: &Protocol,
    select_clause: Option<&str>,
    only_active: bool,
    resolved_to: bool,
    owned_by: bool,
) -> SelectStatement {
    let mut query = if let Some(select_clause) = select_clause {
        sql_gen::domain_select_custom(protocol, select_clause)
    } else {
        sql_gen::domain_select(protocol)
    };

    let mut q = query
        .with_block_range()
        .with_non_empty_label()
        .with_resolved_names();
    if only_active {
        q = q.with_not_expired();
    };

    // Trick: in resolved_to and owned_by are not provided, binding still exists and `cond` will be false
    let mut main_cond = Condition::any().add(Expr::cust("$1 <> $1"));
    if resolved_to {
        main_cond = main_cond.add(Expr::cust("resolved_address = $1"));
    }
    if owned_by {
        main_cond = main_cond.add(Expr::cust("owner = $1"));
        main_cond = main_cond.add(Expr::cust("wrapped_owner = $1"));
    }
    q = q.cond_where(main_cond);

    q.to_owned()
}
