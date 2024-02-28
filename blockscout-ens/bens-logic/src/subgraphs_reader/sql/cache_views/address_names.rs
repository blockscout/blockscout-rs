use super::CachedView;
use crate::{
    entity::subgraph::domain::DomainWithAddress,
    subgraphs_reader::{
        sql::{
            bind_string_list, DOMAIN_BLOCK_RANGE_WHERE_CLAUSE, DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE,
            DOMAIN_NOT_EXPIRED_WHERE_CLAUSE,
        },
        SubgraphReadError,
    },
};
use sqlx::PgPool;
use tracing::instrument;

pub struct AddressNamesView;

#[async_trait::async_trait]
impl CachedView for AddressNamesView {
    fn refresh_function_name() -> &'static str {
        "refresh_address_names()"
    }

    fn view_table_name() -> &'static str {
        "address_names"
    }

    fn unique_field() -> &'static str {
        "resolved_address"
    }

    fn table_sql(schema: &str) -> String {
        format!(
            r#"
        SELECT DISTINCT ON (resolved_address)
            id,
            name AS domain_name,
            resolved_address
        from {schema}.domain
        where
            resolved_address IS NOT NULL
            AND name NOT LIKE '%[%'
            AND {DOMAIN_BLOCK_RANGE_WHERE_CLAUSE}
            AND {DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE}
            AND {DOMAIN_NOT_EXPIRED_WHERE_CLAUSE}
        ORDER BY resolved_address, created_at"#
        )
    }
}

impl AddressNamesView {
    // TODO: rewrite to sea_query generation
    #[instrument(
        name = "AddressNamesView::batch_search_addresses",
        skip(pool, addresses),
        fields(job_size = addresses.len()),
        err(level = "error"),
        level = "info",
    )]
    pub async fn batch_search_addresses(
        pool: &PgPool,
        schema: &str,
        addresses: &[impl AsRef<str>],
    ) -> Result<Vec<DomainWithAddress>, SubgraphReadError> {
        let view_table_name = Self::view_table_name();
        let domains: Vec<DomainWithAddress> = sqlx::query_as(&format!(
            r#"
            SELECT id, domain_name, resolved_address
            FROM {schema}.{view_table_name}
            where
                resolved_address = ANY($1)
            "#
        ))
        .bind(bind_string_list(addresses))
        .fetch_all(pool)
        .await?;

        Ok(domains)
    }
}
