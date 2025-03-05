use crate::{error::ServiceError, repository, types::chains::Chain};
use cached::proc_macro::cached;
use sea_orm::DatabaseConnection;

#[cached(
    key = "bool",
    convert = r#"{ only_active }"#,
    time = 600, // 10 minutes
    result = true
)]
pub async fn list_chains_cached(
    db: &DatabaseConnection,
    only_active: bool,
) -> Result<Vec<Chain>, ServiceError> {
    let chains = repository::chains::list_chains(db, only_active)
        .await?
        .into_iter()
        .map(|c| c.into())
        .collect();
    Ok(chains)
}
