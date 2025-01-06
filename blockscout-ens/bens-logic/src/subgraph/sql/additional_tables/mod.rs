mod addr2name;
mod cache_views;
pub use addr2name::*;
pub use cache_views::*;

use sqlx::{Executor, PgPool};

use super::DbErr;

#[async_trait::async_trait]
pub trait AdditionalTable {
    fn table_name() -> &'static str;
    fn create_table_sql(schema: &str) -> String;

    async fn create_table(pool: &PgPool, schema: &str) -> Result<(), DbErr> {
        pool.execute(sqlx::query(&Self::create_table_sql(schema)))
            .await?;
        Ok(())
    }
}
