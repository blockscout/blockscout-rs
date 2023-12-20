use super::sql;
use sqlx::PgPool;
use std::sync::Arc;

pub async fn fix_domain_name(pool: Arc<PgPool>, schema: &str, domain_name: &str, domain_id: &str) {
    match sql::update_domain_name(pool.as_ref(), schema, domain_id, domain_name).await {
        Ok(r) => {
            tracing::info!(
                rows_affected = r.rows_affected(),
                name = domain_name,
                "successfuly updated domain name"
            );
        }
        Err(err) => tracing::error!(name = domain_name, "cannot update domain name: {err}"),
    }
}
