// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::{
    entity::subgraph::domain::{DetailedDomain, Domain},
    protocols::{hash_name::hex, DomainNameOnProtocol, Protocol},
    subgraph::{
        sql::{utils, DbErr},
        DomainPaginationInput, LookupAddressInput,
    },
};
use alloy::primitives::Address;
use nonempty::NonEmpty;
use sea_query::{Alias, Condition, Expr, PostgresQueryBuilder, SelectStatement};
use sql_gen::QueryBuilderExt;
use sqlx::postgres::PgPool;
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
        domain_select_custom(protocol, &detailed_domain_select_clause(protocol))
    }

    pub fn domain_select(protocol: &Protocol) -> SelectStatement {
        domain_select_custom(protocol, &domain_select_clause(protocol))
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

// Subgraphs store `ttl` and `expiry_date` as raw uint64 seconds, and names that
// never expire carry type(uint64).max (~1.8e19). `to_timestamp` rejects such
// values with "timestamp out of range" (22008), failing the whole query for
// every domain it scans. Values past 9999-12-31T23:59:59Z (253402300799) are
// therefore clamped in filters and returned as NULL ("no expiry").
// ensdomains/ens-subgraph 2.1.1+ clamps never-expiring names to exactly
// 253402300799 on `expiry_date`, so that value itself is "no expiry" too.

fn detailed_domain_select_clause(protocol: &Protocol) -> String {
    let owner = owner_select_expr(protocol);
    let ensv2_migrated = ensv2_migrated_expr(protocol);
    format!(
        r#"
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
CASE WHEN ttl <= 253402300799 THEN to_timestamp(ttl) END as ttl,
is_migrated,
stored_offchain,
resolved_with_wildcard,
created_at,
to_timestamp(created_at) as registration_date,
{owner} as owner,
owner as registry_owner,
{ensv2_migrated} as ensv2_migrated,
registrant,
wrapped_owner,
CASE WHEN expiry_date < 253402300799 THEN to_timestamp(expiry_date) END as expiry_date,
COALESCE(to_timestamp(LEAST(expiry_date, 253402300799)) < now(), false) AS is_expired,
token_id::text as token_id
"#
    )
}

fn domain_select_clause(protocol: &Protocol) -> String {
    let owner = owner_select_expr(protocol);
    format!(
        r#"
vid,
id,
name,
resolved_address,
resolver,
created_at,
to_timestamp(created_at) as registration_date,
{owner} as owner,
wrapped_owner,
stored_offchain,
resolved_with_wildcard,
CASE WHEN expiry_date < 253402300799 THEN to_timestamp(expiry_date) END as expiry_date,
COALESCE(to_timestamp(LEAST(expiry_date, 253402300799)) < now(), false) AS is_expired
"#
    )
}

// ENSv2 (ensdomains/ens-subgraph 2.x) keeps `Domain.owner` as the ENSv1
// registry owner. For a migrated name that is either a migration contract
// (the Graveyard or a migration controller: the migration's voiding step for
// an unwrapped name or an unlocked wrapped one) or still the NameWrapper (a
// locked migration leaves the name wrapped and parks the ERC-1155 token in
// the Graveyard). Neither is the holder. The holder is the wrapped owner
// (locked migration), the registrant (unlocked .eth migration) or, for
// migrated subnames that have neither, the owner of the ENSv2 name slot the
// Domain is projected from (`ENSv2NamePath.domain` -> `ENSv2NameSlot.owner`).
//
// `migration_contracts` is empty for every protocol without `ensv2` in its
// config, so other ENS-like subgraphs (which have no ENSv2 tables) keep the
// plain column.
fn owner_select_expr(protocol: &Protocol) -> String {
    match protocol.info.protocol_specific.ensv2() {
        Some(v2) if !v2.migration_contracts.is_empty() => {
            let schema = &protocol.subgraph_schema;
            let contracts = migration_contracts_array(&v2.migration_contracts);
            // A locked migration can park the v1 BaseRegistrar token in the
            // Graveyard as well, so a registrant (or wrapped owner) that is a
            // migration contract is skipped like the owner. The wrapped-owner
            // check for the NameWrapper case is a cheap guard in front of the
            // slot lookup: a migrated name whose registry owner is not a
            // migration contract is always a locked (wrapped) one.
            format!(
                "(CASE WHEN owner = ANY({contracts}) \
                 OR (wrapped_owner IS NOT NULL AND {ENSV2_MIGRATED}) THEN (CASE \
                 WHEN wrapped_owner IS NOT NULL AND NOT (wrapped_owner = ANY({contracts})) THEN wrapped_owner \
                 WHEN registrant IS NOT NULL AND NOT (registrant = ANY({contracts})) THEN registrant \
                 ELSE COALESCE(({ENSV2_SLOT_OWNER_SUBQUERY}), owner) END) ELSE owner END)",
                ENSV2_MIGRATED = ensv2_migrated_subquery(schema),
                ENSV2_SLOT_OWNER_SUBQUERY = ensv2_slot_owner_subquery(schema),
            )
        }
        _ => "owner".to_string(),
    }
}

// Whether the current domain row is an ENSv1 name migrated to ENSv2
// (`ENSv2NameSlot.migratedFromV1` of its active slot). `false` for protocols
// without `ensv2`, which have no ENSv2 tables.
fn ensv2_migrated_expr(protocol: &Protocol) -> String {
    match protocol.info.protocol_specific.ensv2() {
        Some(v2) if !v2.migration_contracts.is_empty() => {
            ensv2_migrated_subquery(&protocol.subgraph_schema)
        }
        _ => "false".to_string(),
    }
}

fn ensv2_migrated_subquery(schema: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM {schema}.en_sv_2_name_path p \
         JOIN {schema}.en_sv_2_name_slot s ON s.id = p.slot \
         WHERE p.id = decode(substr(domain.id, 3), 'hex') AND p.active AND s.migrated_from_v1 \
         AND p.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE} AND s.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE})"
    )
}

fn migration_contracts_array(contracts: &[Address]) -> String {
    let items = contracts
        .iter()
        .map(|c| format!("'{}'", hex(c)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("ARRAY[{items}]::text[]")
}

// Owner of the active ENSv2 name slot behind the current domain row.
// `ENSv2NamePath.id` is the namehash as bytes, `Domain.id` the same hash as a
// 0x-prefixed text, hence the decode.
fn ensv2_slot_owner_subquery(schema: &str) -> String {
    format!(
        "SELECT s.owner FROM {schema}.en_sv_2_name_path p \
         JOIN {schema}.en_sv_2_name_slot s ON s.id = p.slot \
         WHERE p.id = decode(substr(domain.id, 3), 'hex') AND p.active \
         AND p.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE} AND s.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE} \
         LIMIT 1"
    )
}

// Domains whose active ENSv2 name slot is owned by `$1`, for the owned-by
// filter; the current domain row and the slot are matched by namehash.
fn ensv2_slot_owned_by_subquery(schema: &str) -> String {
    format!(
        "SELECT '0x' || encode(p.id, 'hex') FROM {schema}.en_sv_2_name_path p \
         JOIN {schema}.en_sv_2_name_slot s ON s.id = p.slot \
         WHERE s.owner = $1 AND p.active \
         AND p.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE} AND s.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}"
    )
}

// `block_range @>` is special sql syntax for fast filtering int4range
// to access current version of domain.
// Source: https://github.com/graphprotocol/graph-node/blob/19fd41bb48511f889dc94f5d82e16cd492f29da1/store/postgres/src/block_range.rs#L26
pub const DOMAIN_BLOCK_RANGE_WHERE_CLAUSE: &str = "block_range @> 2147483647";

pub const DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE: &str = "label_name IS NOT NULL";

// See `detailed_domain_select_clause` for the clamp.
pub const DOMAIN_NOT_EXPIRED_WHERE_CLAUSE: &str = r#"
(
    expiry_date is null
    OR to_timestamp(LEAST(expiry_date, 253402300799)) > now()
)
"#;

// TODO: rewrite to sea_query generation
#[instrument(
    skip_all,
    err(level = "error"),
    level = "info",
    fields(
        domain_name = %domain_name.inner.name(),
        protocol_slug = %domain_name.deployed_protocol.protocol.info.slug)
    )
]
pub async fn get_domain(
    pool: &PgPool,
    domain_name: &DomainNameOnProtocol<'_>,
    only_active: bool,
) -> Result<Option<DetailedDomain>, DbErr> {
    let only_active_clause = if only_active {
        format!("AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}")
    } else {
        String::new()
    };
    let schema = &domain_name.deployed_protocol.protocol.subgraph_schema;
    let protocol_slug = &domain_name.deployed_protocol.protocol.info.slug;
    let detailed_select_clause =
        detailed_domain_select_clause(domain_name.deployed_protocol.protocol);
    let maybe_domain = sqlx::query_as(&format!(
        r#"
        SELECT
            {detailed_select_clause},
            '{schema}' as schema_name,
            '{protocol_slug}' as protocol_slug,
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
    .bind(domain_name.inner.id())
    .fetch_optional(pool)
    .await?;
    Ok(maybe_domain)
}

#[derive(Clone, Debug)]
pub enum FindDomainsInput<'a> {
    Names(Vec<DomainNameOnProtocol<'a>>),
    Protocols(Vec<&'a Protocol>),
}

#[instrument(skip_all, err(level = "error"), level = "info")]
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
                .map(|name| {
                    (
                        name.deployed_protocol.protocol.subgraph_schema.clone(),
                        name.deployed_protocol,
                    )
                })
                .collect::<HashMap<_, _>>();
            unique_protocols
                .values()
                .map(|protocol| {
                    let mut query = sql_gen::domain_select(protocol.protocol);
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
        query = query.bind(names.iter().map(|n| n.inner.id()).collect::<Vec<_>>());
    }
    let domains = query.fetch_all(pool).await?;
    Ok(domains)
}

#[instrument(
    skip_all,
    err(level = "error"),
    level = "info",
    fields(address = %input.address))
]
pub async fn find_resolved_addresses(
    pool: &PgPool,
    protocols: NonEmpty<&Protocol>,
    input: &LookupAddressInput,
) -> Result<Vec<Domain>, DbErr> {
    // Without at least one of these the address predicate renders as a bare
    // FALSE, which drops $1 from the SQL and breaks the bind below.
    if !input.resolved_to && !input.owned_by {
        return Ok(Vec::new());
    }
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
    skip_all,
    err(level = "error"),
    level = "info",
    fields(address = %address))
]
pub async fn count_domains_by_address(
    pool: &PgPool,
    protocols: NonEmpty<&Protocol>,
    address: Address,
    only_active: bool,
    resolved_to: bool,
    owned_by: bool,
) -> Result<i64, DbErr> {
    // See `find_resolved_addresses`.
    if !resolved_to && !owned_by {
        return Ok(0);
    }
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

    // No `$1 <> $1` sentinel here: it is opaque to the planner, so in a generic
    // plan Postgres estimates the whole disjunction at ~99.5% selectivity, gives
    // up on the resolved_address/owner/wrapped_owner indexes and walks the
    // created_at index instead. Callers guarantee at least one flag is set.
    debug_assert!(
        resolved_to || owned_by,
        "caller must reject the empty filter before building the query"
    );
    let mut main_cond = Condition::any();
    if resolved_to {
        main_cond = main_cond.add(Expr::cust("resolved_address = $1"));
    }
    if owned_by {
        let mut owned = Condition::any()
            .add(Expr::cust("owner = $1"))
            .add(Expr::cust("wrapped_owner = $1"))
            // The .eth registrant: for an ENSv2-migrated (unlocked) name the
            // registry owner is a migration contract and the registrant is the
            // holder; for an ENSv1 name it is the BaseRegistrar token holder.
            .add(Expr::cust("registrant = $1"));
        match protocol.info.protocol_specific.ensv2() {
            Some(v2) => {
                // Migrated subnames carry neither registrant nor wrapped
                // owner; their holder is the ENSv2 slot owner.
                owned = owned.add(Expr::cust(format!(
                    "id = ANY({})",
                    ensv2_slot_owned_by_subquery(&protocol.subgraph_schema)
                )));
                if !v2.migration_contracts.is_empty() {
                    // A migration contract is the v1 registry owner or
                    // registrant of every migrated name but holds none of
                    // them, so its own address page lists nothing. The guard
                    // only compares the parameter with constants, so the
                    // indexable disjuncts above are unaffected.
                    main_cond =
                        main_cond.add(Condition::all().add(owned).add(Expr::cust(format!(
                            "NOT ($1 = ANY({}))",
                            migration_contracts_array(&v2.migration_contracts)
                        ))));
                } else {
                    main_cond = main_cond.add(owned);
                }
            }
            None => {
                main_cond = main_cond.add(owned);
            }
        }
    }
    q = q.cond_where(main_cond);

    q.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::{EnsLikeProtocol, EnsV2Settings, ProtocolSpecific};
    use std::str::FromStr;

    fn protocol() -> Protocol {
        Protocol {
            subgraph_schema: "sgd1".to_string(),
            ..Default::default()
        }
    }

    const GRAVEYARD: &str = "0xda021a2c1c2f99574d861ef8c6fa745851beb7f6";

    fn ensv2_protocol() -> Protocol {
        let mut protocol = protocol();
        protocol.info.protocol_specific = ProtocolSpecific::EnsLike(EnsLikeProtocol {
            ensv2: Some(EnsV2Settings {
                migration_contracts: vec![Address::from_str(GRAVEYARD).unwrap()],
            }),
            ..Default::default()
        });
        protocol
    }

    fn address_filter_sql(resolved_to: bool, owned_by: bool) -> String {
        gen_sql_select_domains_by_address(&protocol(), None, true, resolved_to, owned_by)
            .to_string(PostgresQueryBuilder)
    }

    #[test]
    fn expiry_past_year_9999_is_reported_as_never() {
        for clause in [
            detailed_domain_select_clause(&protocol()),
            domain_select_clause(&protocol()),
        ] {
            assert!(
                clause.contains(
                    "CASE WHEN expiry_date < 253402300799 THEN to_timestamp(expiry_date) END as expiry_date"
                ),
                "{clause}"
            );
            assert!(
                clause.contains("COALESCE(to_timestamp(LEAST(expiry_date, 253402300799)) < now(), false) AS is_expired"),
                "{clause}"
            );
        }
    }

    #[test]
    fn owner_is_plain_column_without_ensv2() {
        let sql = sql_gen::domain_select(&protocol()).to_string(PostgresQueryBuilder);
        assert!(sql.contains("owner as owner"), "{sql}");
        assert!(!sql.contains("CASE WHEN owner"), "{sql}");
        assert!(!sql.contains("en_sv_2"), "{sql}");

        let detailed = detailed_domain_select_clause(&protocol());
        assert!(detailed.contains("owner as registry_owner"), "{detailed}");
        assert!(detailed.contains("false as ensv2_migrated"), "{detailed}");
    }

    #[test]
    fn owner_substitutes_migration_contracts_with_ensv2() {
        let sql = sql_gen::domain_select(&ensv2_protocol()).to_string(PostgresQueryBuilder);
        let contracts = format!("ARRAY['{GRAVEYARD}']::text[]");
        let migrated = "EXISTS (SELECT 1 FROM sgd1.en_sv_2_name_path p \
                        JOIN sgd1.en_sv_2_name_slot s ON s.id = p.slot \
                        WHERE p.id = decode(substr(domain.id, 3), 'hex') AND p.active AND s.migrated_from_v1 \
                        AND p.block_range @> 2147483647 AND s.block_range @> 2147483647)";
        assert!(
            sql.contains(&format!(
                "CASE WHEN owner = ANY({contracts}) \
                 OR (wrapped_owner IS NOT NULL AND {migrated}) THEN (CASE \
                 WHEN wrapped_owner IS NOT NULL AND NOT (wrapped_owner = ANY({contracts})) THEN wrapped_owner \
                 WHEN registrant IS NOT NULL AND NOT (registrant = ANY({contracts})) THEN registrant \
                 ELSE COALESCE((SELECT s.owner FROM sgd1.en_sv_2_name_path p"
            )),
            "{sql}"
        );
        assert!(sql.contains("ELSE owner END) as owner"), "{sql}");

        let detailed = detailed_domain_select_clause(&ensv2_protocol());
        assert!(detailed.contains("owner as registry_owner"), "{detailed}");
        assert!(
            detailed.contains(&format!("{migrated} as ensv2_migrated")),
            "{detailed}"
        );
    }

    #[test]
    fn owned_by_includes_ensv2_slot_owner_only_with_ensv2() {
        let plain = address_filter_sql(false, true);
        assert!(!plain.contains("en_sv_2_name_slot"), "{plain}");

        let v2 = gen_sql_select_domains_by_address(&ensv2_protocol(), None, true, false, true)
            .to_string(PostgresQueryBuilder);
        assert!(v2.contains("(registrant = $1)"), "{v2}");
        assert!(
            v2.contains(
                "(id = ANY(SELECT '0x' || encode(p.id, 'hex') FROM sgd1.en_sv_2_name_path p"
            ),
            "{v2}"
        );
        assert!(v2.contains("WHERE s.owner = $1"), "{v2}");
        assert!(
            v2.contains(&format!(
                "AND (NOT ($1 = ANY(ARRAY['{GRAVEYARD}']::text[])))"
            )),
            "{v2}"
        );
        // the subquery must not introduce a second bind parameter
        assert!(!v2.contains("$2"), "{v2}");
    }

    #[test]
    fn address_filter_stays_indexable() {
        // A `$1 <> $1` disjunct is opaque to the planner: in a generic plan it
        // pushes the estimate for the whole OR to ~99.5% of the table, the
        // attribute indexes stop being used and the query degrades into a full
        // walk of the created_at index.
        for (resolved_to, owned_by) in [(true, true), (true, false), (false, true)] {
            let sql = address_filter_sql(resolved_to, owned_by);
            assert!(!sql.contains("$1 <> $1"), "{sql}");
            // callers bind exactly one parameter, so $1 must survive every variant
            assert!(sql.contains("$1"), "{sql}");
        }
    }

    #[test]
    fn address_filter_respects_flags() {
        let both = address_filter_sql(true, true);
        assert!(both.contains("(resolved_address = $1)"), "{both}");
        assert!(both.contains("(owner = $1)"), "{both}");
        assert!(both.contains("(wrapped_owner = $1)"), "{both}");
        assert!(both.contains("(registrant = $1)"), "{both}");

        let resolved_only = address_filter_sql(true, false);
        assert!(
            resolved_only.contains("(resolved_address = $1)"),
            "{resolved_only}"
        );
        assert!(!resolved_only.contains("(owner = $1)"), "{resolved_only}");
        assert!(
            !resolved_only.contains("(wrapped_owner = $1)"),
            "{resolved_only}"
        );
        assert!(
            !resolved_only.contains("(registrant = $1)"),
            "{resolved_only}"
        );

        let owned_only = address_filter_sql(false, true);
        assert!(
            !owned_only.contains("(resolved_address = $1)"),
            "{owned_only}"
        );
        assert!(owned_only.contains("(owner = $1)"), "{owned_only}");
        assert!(owned_only.contains("(wrapped_owner = $1)"), "{owned_only}");
        assert!(owned_only.contains("(registrant = $1)"), "{owned_only}");
    }
}
