use super::AdditionalTable;
use crate::{
    entity::subgraph::domain::DomainWithAddress,
    protocols::Protocol,
    subgraph::sql::{utils, DbErr},
};
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
use sqlx::PgPool;

pub struct Addr2NameTable;

#[async_trait::async_trait]
impl AdditionalTable for Addr2NameTable {
    fn table_name() -> &'static str {
        "addr2name"
    }

    fn create_table_sql(schema: &str) -> String {
        let table_name = Self::table_name();
        format!(
            r#"
            CREATE TABLE IF NOT EXISTS {schema}.{table_name} (
                resolved_address TEXT PRIMARY KEY,
                domain_id TEXT,
                domain_name TEXT
            );
        "#
        )
    }
}

impl Addr2NameTable {
    pub async fn batch_search_addreses(
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
                .from((Alias::new(&p.subgraph_schema), Alias::new(table_name)))
                .and_where(Expr::cust("resolved_address = ANY($1)"))
                .and_where(Expr::cust("domain_id is not null"))
                .and_where(Expr::cust("domain_name is not null"))
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
