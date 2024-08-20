use anyhow::Context;
use sqlx::{Executor, PgPool, Row};

mod addr_reverse_names;
mod address_names;

use crate::subgraph::sql::DbErr;
pub use addr_reverse_names::AddrReverseNamesView;
pub use address_names::AddressNamesView;

#[async_trait::async_trait]
pub trait CachedView {
    fn refresh_function_name() -> &'static str;
    fn view_table_name() -> &'static str;
    fn unique_field() -> &'static str;
    fn table_sql(schema: &str) -> String;

    async fn create_view(pool: &PgPool, schema: &str) -> Result<(), DbErr> {
        let view_table_name = Self::view_table_name();
        let refresh_function_name = Self::refresh_function_name();
        let unique_field = Self::unique_field();
        let table_sql = Self::table_sql(schema);
        let mut tx = pool.begin().await?;

        // https://stackoverflow.com/questions/20582500/how-to-check-if-a-table-exists-in-a-given-schema
        let exists = tx
            .fetch_one(sqlx::query(&format!(
                "SELECT to_regclass('{schema}.{view_table_name}') is not null;",
            )))
            .await?
            .try_get::<bool, _>(0)
            .context("checking if view exists")?;
        if exists {
            tracing::info!("view {} already exists, skipping creation", view_table_name);
            return Ok(());
        }

        tx.execute(sqlx::query(&format!(
            r#"
            CREATE MATERIALIZED VIEW IF NOT EXISTS {schema}.{view_table_name} AS
            {table_sql}
            "#,
        )))
        .await
        .context("creating materialized view")?;
        let index_name = format!("{}_unique_{}", view_table_name, unique_field);
        tx.execute(sqlx::query(&format!(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS
            {index_name}
            ON {schema}.{view_table_name} ({unique_field});
            "#
        )))
        .await
        .context("creating index")?;
        tx.execute(sqlx::query(&format!(
            r#"
            CREATE OR REPLACE FUNCTION {schema}.{refresh_function_name}
            RETURNS void AS
            $$
            BEGIN
                REFRESH MATERIALIZED VIEW CONCURRENTLY {schema}.{view_table_name};
            END;
            $$
            LANGUAGE plpgsql;
            "#
        )))
        .await
        .context("creating replace function")?;

        tx.commit().await?;

        Ok(())
    }

    async fn refresh_view(pool: &PgPool, schema: &str) -> Result<(), DbErr> {
        let refresh_function_name = Self::refresh_function_name();
        sqlx::query(&format!("SELECT {schema}.{refresh_function_name};"))
            .execute(pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{Executor, PgPool, Row};

    struct TestView;

    impl CachedView for TestView {
        fn refresh_function_name() -> &'static str {
            "refresh_test_view()"
        }

        fn view_table_name() -> &'static str {
            "test_view"
        }

        fn unique_field() -> &'static str {
            "max_int"
        }

        fn table_sql(schema: &str) -> String {
            format!("SELECT bar as max_int FROM {schema}.foo ORDER BY bar DESC LIMIT 1")
        }
    }

    #[sqlx::test(migrations = false)]
    async fn cache_view_works(pool: PgPool) -> anyhow::Result<()> {
        let mut conn = pool.acquire().await?;
        conn.execute("CREATE SCHEMA sgd1;").await?;
        conn.execute("CREATE TABLE sgd1.foo(bar integer)").await?;
        conn.execute("INSERT INTO sgd1.foo VALUES (1)").await?;

        TestView::create_view(&pool, "sgd1").await?;
        assert_current_max_is(&pool, 1).await;
        conn.execute("INSERT INTO sgd1.foo VALUES (100)").await?;
        assert_current_max_is(&pool, 1).await;
        TestView::refresh_view(&pool, "sgd1").await?;
        assert_current_max_is(&pool, 100).await;
        Ok(())
    }

    async fn assert_current_max_is(pool: &PgPool, max_int: i32) {
        let row = sqlx::query("SELECT * FROM sgd1.test_view")
            .fetch_one(pool)
            .await
            .expect("fetching test view");
        assert_eq!(row.get::<i32, _>("max_int"), max_int);
    }
}
