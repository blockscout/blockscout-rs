use crate::{protocols::DomainName, subgraph::sql::DbErr};
use sqlx::{postgres::PgQueryResult, PgPool};
use tracing::instrument;

#[instrument(
    name = "update_domain_name",
    skip(pool),
    err(level = "error"),
    level = "info"
)]
pub async fn update_domain_name(
    pool: &PgPool,
    schema: &str,
    name: DomainName,
) -> Result<PgQueryResult, DbErr> {
    let result = sqlx::query(&format!(
        "UPDATE {schema}.domain SET name = $1, label_name = $2 WHERE id = $3;"
    ))
    .bind(&name.name)
    .bind(&name.label_name)
    .bind(&name.id)
    .execute(pool)
    .await?;
    Ok(result)
}
