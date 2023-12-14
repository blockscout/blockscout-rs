use super::sql;
use crate::hash_name::domain_id;
use sqlx::PgPool;
use std::sync::Arc;

pub async fn fix_domain_name(pool: Arc<PgPool>, schema: &str, domain_name: &str) {
    let id = domain_id(domain_name);
    match sql::update_domain_name(pool.as_ref(), schema, &id, domain_name).await {
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
