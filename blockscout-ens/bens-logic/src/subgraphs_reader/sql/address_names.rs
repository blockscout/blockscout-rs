use crate::subgraphs_reader::SubgraphReadError;
use sqlx::{postgres::PgPool, Executor};
use tracing::instrument;

#[instrument(
    name = "create_address_names_view",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn create_address_names_view(
    pool: &PgPool,
    schema: &str,
) -> Result<(), SubgraphReadError> {
    let mut tx = pool.begin().await?;

    tx.execute(sqlx::query(&format!(
        r#"
        CREATE MATERIALIZED VIEW IF NOT EXISTS {schema}.address_names AS
        SELECT DISTINCT ON (resolved_address)
            id,
            name AS domain_name,
            resolved_address
        from {schema}.domain
        where
            resolved_address IS NOT NULL
            AND label_name IS NOT NULL
            AND name NOT LIKE '%[%'
            AND (
                expiry_date is null
                OR to_timestamp(expiry_date) > now()
            )
            AND block_range @> 2147483647
        ORDER BY resolved_address, created_at
        "#,
    )))
    .await?;

    tx.execute(sqlx::query(&format!(
        r#"
        CREATE UNIQUE INDEX IF NOT EXISTS
        address_names_unique_resolved_address
        ON {schema}.address_names (resolved_address);
        "#
    )))
    .await?;

    let function_name = refresh_function_name(schema);
    tx.execute(sqlx::query(&format!(
        r#"
        CREATE OR REPLACE FUNCTION {function_name}
        RETURNS void AS
        $$
        BEGIN
            REFRESH MATERIALIZED VIEW CONCURRENTLY {schema}.address_names;
        END;
        $$
        LANGUAGE plpgsql;
        "#
    )))
    .await?;

    tx.commit().await?;

    Ok(())
}

#[instrument(
    name = "refresh_address_names_view",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn refresh_address_names_view(
    pool: &PgPool,
    schema: &str,
) -> Result<(), SubgraphReadError> {
    let function_name = refresh_function_name(schema);
    sqlx::query(&format!("SELECT {function_name};"))
        .execute(pool)
        .await?;
    Ok(())
}

fn refresh_function_name(schema: &str) -> String {
    format!("{schema}_refresh_address_names()")
}
