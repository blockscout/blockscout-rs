use super::CachedView;
use crate::{
    entity::subgraph::domain::DomainWithAddress,
    protocols::Protocol,
    subgraph::sql::{
        utils, DbErr, DOMAIN_BLOCK_RANGE_WHERE_CLAUSE, DOMAIN_NONEMPTY_LABEL_WHERE_CLAUSE,
        DOMAIN_NOT_EXPIRED_WHERE_CLAUSE,
    },
};
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
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
        skip_all,
        fields(
            job_size = addresses.len(),
            protocols_size = protocols.len(),
            fist_protocol_schema = protocols.head.subgraph_schema),
        err(level = "error"),
        level = "info",
    )]
    pub async fn batch_search_addresses(
        pool: &PgPool,
        protocols: &NonEmpty<&Protocol>,
        addresses: &[impl AsRef<str>],
    ) -> Result<Vec<DomainWithAddress>, DbErr> {
        let view_table_name = Self::view_table_name();
        let queries = NonEmpty::collect(protocols.into_iter().map(|p| {
            sea_query::Query::select()
                .expr(Expr::cust("id"))
                .expr(Expr::cust("domain_name"))
                .expr(Expr::cust("resolved_address"))
                .from((Alias::new(&p.subgraph_schema), Alias::new(view_table_name)))
                .and_where(Expr::cust("resolved_address = ANY($1)"))
                .to_owned()
        }))
        .expect("protocols is nonempty");
        let sql = utils::union_domain_queries(queries, None, None)?.to_string(PostgresQueryBuilder);
        let domains = sqlx::query_as(&sql)
            .bind(utils::bind_string_list(addresses))
            .fetch_all(pool)
            .await?;
        Ok(domains)
    }
}
