use crate::{
    entity::subgraph::domain::DomainWithAddress,
    protocols::Protocol,
    subgraph::sql::{utils, DbErr, DOMAIN_BLOCK_RANGE_WHERE_CLAUSE},
};
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
use sqlx::PgPool;

pub struct PrimaryNameRecordTable;

impl PrimaryNameRecordTable {
    fn table_name() -> &'static str {
        "primary_name_record"
    }
}

impl PrimaryNameRecordTable {
    pub async fn batch_search_addresses(
        pool: &PgPool,
        protocols: &NonEmpty<&Protocol>,
        address: &[impl AsRef<str>],
    ) -> Result<Vec<DomainWithAddress>, DbErr> {
        let table_name = Self::table_name();
        let queries = NonEmpty::collect(protocols.into_iter().map(|p| {
            sea_query::Query::select()
                .expr(Expr::cust("domain_id as id"))
                .expr(Expr::cust("domain_name"))
                .expr(Expr::cust("resolved_address"))
                .expr(Expr::cust(format!("'{}' as protocol_slug", p.info.slug)))
                .from((Alias::new(&p.subgraph_schema), Alias::new(table_name)))
                .and_where(Expr::cust("resolved_address = ANY($1)"))
                .and_where(Expr::cust("domain_id is not null"))
                .and_where(Expr::cust("domain_name is not null"))
                .and_where(Expr::cust(DOMAIN_BLOCK_RANGE_WHERE_CLAUSE))
                .to_owned()
        }))
        .expect("protocols is nonempty");
        let sql = utils::union_domain_queries(queries, None, None)?.to_string(PostgresQueryBuilder);
        let domains = sqlx::query_as(&sql)
            .bind(utils::bind_string_list(address))
            .fetch_all(pool)
            .await?;
        Ok(domains)
    }
}
