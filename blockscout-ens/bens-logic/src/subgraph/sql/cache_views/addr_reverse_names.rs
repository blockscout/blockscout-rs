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
        // Filter all domain that has
        // parent = hashname('addr.reverse') = 0x91d1777781884d03a6757a803996e38de2a42967fb37eeaca72729271025a9e2
        format!(
            r#"
        SELECT
            domain.id as domain_id,
            addr_reversed_domain.id as reversed_domain_id,
            domain.resolved_address as resolved_address,
            nc.name as name
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
        AND nc.{DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
        AND (
            domain.expiry_date is null
            OR to_timestamp(domain.expiry_date) > now()
        )
        "#
        )
    }
}

impl AddrReverseNamesView {
    #[instrument(
        name = "AddrReverseNamesView::batch_search_addresses",
        skip_all,
        fields(
            job_size = address_hashes.len(),
            protocols_size = protocols.len(),
            fist_protocol_schema = protocols.head.subgraph_schema,
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
                .from((Alias::new(&p.subgraph_schema), Alias::new(view_table_name)))
                .and_where(Expr::cust("reversed_domain_id = ANY($1)"))
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
