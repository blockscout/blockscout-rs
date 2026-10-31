// SPDX-License-Identifier: LicenseRef-Blockscout

use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
use sqlx::PgPool;
use tracing::instrument;

use super::CachedView;
use crate::{
    entity::subgraph::domain::AddrReverseDomainWithActualName,
    protocols::Protocol,
    subgraph::sql::{utils, DbErr, DOMAIN_BLOCK_RANGE_WHERE_CLAUSE},
};

pub struct AddrReverseNamesView;

#[async_trait::async_trait]
impl CachedView for AddrReverseNamesView {
    fn refresh_function_name() -> &'static str {
        "refresh_addr_reverse_names()"
    }

    fn view_table_name() -> &'static str {
        "addr_reverse_names"
    }

    fn unique_field() -> &'static str {
        "reversed_domain_id"
    }

    fn table_sql(schema: &str) -> String {
        Self::sql(schema, false)
    }

    fn table_sql_for_protocol(protocol: &Protocol) -> String {
        Self::sql(
            &protocol.subgraph_schema,
            protocol.info.protocol_specific.ensv2().is_some(),
        )
    }
}

impl AddrReverseNamesView {
    // Reverse (address -> name) records. Two sources:
    //  * v1 (every ENS-like subgraph): NameChanged on the resolver of
    //    <addr>.addr.reverse. `name_changed` is an immutable entity in
    //    ensdomains/ens-subgraph 2.x (`block$` column, no `block_range`), so it
    //    carries no block-range filter; event rows are never superseded in
    //    the older mutable schema either, so the filter was a no-op there.
    //  * ENSv2 (`ensv2` configured, ensdomains/ens-subgraph 2.x): ENSIP-19
    //    default primary names (DefaultReverseRegistrar `NameForAddrChanged`),
    //    indexed as ENSv2PrimaryName keyed by address with `reverse_node` =
    //    namehash(<addr>.addr.reverse), which is what this view and
    //    resolve_addr_reverse_cached key on.
    // Newest record per reverse node wins.
    // parent = hashname('addr.reverse') = 0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2
    fn sql(schema: &str, ensv2: bool) -> String {
        let v1 = format!(
            r#"
    -- v1 scheme: NameChanged on the resolver of <addr>.addr.reverse
    SELECT
        domain.id as domain_id,
        addr_reversed_domain.id as reversed_domain_id,
        domain.resolved_address as resolved_address,
        nc.name as name,
        nc.block_number as block_number
    FROM (
        SELECT DISTINCT ON (resolver) *
        FROM {schema}.name_changed
        ORDER BY resolver, block_number DESC
    ) nc
    JOIN {schema}.domain addr_reversed_domain ON nc.resolver = addr_reversed_domain.resolver
    JOIN {schema}.domain domain ON domain.name = nc.name
    WHERE true
    AND addr_reversed_domain.parent = '0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2'
    AND addr_reversed_domain.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
    AND domain.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
    AND (
        domain.expiry_date is null
        OR to_timestamp(LEAST(domain.expiry_date, 253402300799)) > now()
    )
"#
        );
        let v2 = if ensv2 {
            format!(
                r#"
    UNION ALL

    -- ENSv2 scheme: ENSIP-19 default primary name (DefaultReverseRegistrar)
    SELECT
        domain.id as domain_id,
        '0x' || encode(pn.reverse_node, 'hex') as reversed_domain_id,
        domain.resolved_address as resolved_address,
        pn.name as name,
        pn.block_number as block_number
    FROM {schema}.en_sv_2_primary_name pn
    JOIN {schema}.domain domain ON domain.name = pn.name
    WHERE pn.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
    AND pn.name IS NOT NULL
    AND pn.reverse_node IS NOT NULL
    AND domain.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
    AND (
        domain.expiry_date is null
        OR to_timestamp(LEAST(domain.expiry_date, 253402300799)) > now()
    )
"#
            )
        } else {
            String::new()
        };
        format!(
            r#"
SELECT DISTINCT ON (reversed_domain_id)
    domain_id, reversed_domain_id, resolved_address, name
FROM (
{v1}{v2}) all_sources
ORDER BY reversed_domain_id, block_number DESC
        "#
        )
    }

    #[instrument(
        name = "AddrReverseNamesView::batch_search_addresses",
        skip_all,
        fields(
            job_size = address_hashes.len(),
            protocols_size = protocols.len(),
            first_protocol_schema = protocols.head.subgraph_schema,
        ),
        err(level = "error"),
        level = "info",
    )]
    pub async fn batch_search_addresses(
        pool: &PgPool,
        protocols: &NonEmpty<&Protocol>,
        address_hashes: &[impl AsRef<str>],
    ) -> Result<Vec<AddrReverseDomainWithActualName>, DbErr> {
        let view_table_name = Self::view_table_name();
        let queries = NonEmpty::collect(protocols.into_iter().map(|p| {
            sea_query::Query::select()
                .expr(Expr::cust("domain_id"))
                .expr(Expr::cust("reversed_domain_id"))
                .expr(Expr::cust("resolved_address"))
                .expr(Expr::cust("name"))
                .expr(Expr::cust(format!("'{}' as protocol_slug", p.info.slug)))
                .from((Alias::new(&p.subgraph_schema), Alias::new(view_table_name)))
                .and_where(Expr::cust("reversed_domain_id = ANY($1)"))
                .and_where(Expr::cust("resolved_address IS NOT NULL"))
                .to_owned()
        }))
        .expect("protocols is nonempty");
        let sql = utils::union_domain_queries(queries, None, None)?.to_string(PostgresQueryBuilder);
        let domains = sqlx::query_as(&sql)
            .bind(utils::bind_string_list(address_hashes))
            .fetch_all(pool)
            .await?;
        Ok(domains)
    }
}
