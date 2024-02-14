use super::CachedView;
use crate::{
    entity::subgraph::domain::AddrReverseDomainWithActualName,
    subgraphs_reader::{sql::bind_string_list, SubgraphReadError},
};
use sqlx::PgPool;
use tracing::instrument;

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
        AND domain.name = nc.name
        AND addr_reversed_domain.block_range @> 2147483647
        AND domain.block_range @> 2147483647
        AND nc.block_range @> 2147483647
        "#
        )
    }
}

impl AddrReverseNamesView {
    #[instrument(
        name = "AddrReverseNamesView::batch_search_addresses",
        skip(pool, address_hashes),
        fields(job_size = address_hashes.len()),
        err(level = "error"),
        level = "info",
    )]
    pub async fn batch_search_addresses(
        pool: &PgPool,
        schema: &str,
        address_hashes: &[impl AsRef<str>],
    ) -> Result<Vec<AddrReverseDomainWithActualName>, SubgraphReadError> {
        let view_table_name = Self::view_table_name();
        let domains: Vec<AddrReverseDomainWithActualName> = sqlx::query_as(&format!(
            r#"
            SELECT *
            FROM {schema}.{view_table_name}
            WHERE reversed_domain_id = ANY($1)
            "#
        ))
        .bind(bind_string_list(address_hashes))
        .fetch_all(pool)
        .await?;
        Ok(domains)
    }
}
