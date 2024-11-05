use super::{utils, DbErr};
use crate::{entity::subgraph::domain::DomainWithAddress, protocols::Protocol};
use nonempty::NonEmpty;
use sea_query::{Alias, Expr, PostgresQueryBuilder};
use sqlx::PgPool;

pub struct Addr2NameTable;

impl Addr2NameTable {
    pub fn view_table_name() -> &'static str {
        "addr2name"
    }

    pub async fn batch_search_addreses(
        pool: &PgPool,
        protocols: &NonEmpty<&Protocol>,
        address: &[impl AsRef<str>],
    ) -> Result<Vec<DomainWithAddress>, DbErr> {
        let view_table_name = Self::view_table_name();
        let queries = NonEmpty::collect(protocols.into_iter().map(|p| {
            sea_query::Query::select()
                .expr(Expr::cust("domain_id as id"))
                .expr(Expr::cust("domain_name"))
                .expr(Expr::cust("resolved_address"))
                .from((Alias::new(&p.subgraph_schema), Alias::new(view_table_name)))
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
