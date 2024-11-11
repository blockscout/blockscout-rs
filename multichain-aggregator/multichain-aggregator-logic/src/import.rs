use crate::{error::ServiceError, repository, types::batch_import_request::BatchImportRequest};
use sea_orm::{DatabaseConnection, TransactionTrait};

pub async fn batch_import(
    db: &DatabaseConnection,
    request: BatchImportRequest,
) -> Result<(), ServiceError> {
    let tx = db.begin().await?;
    repository::addresses::upsert_many(&tx, request.addresses).await?;
    repository::block_ranges::upsert_many(&tx, request.block_ranges).await?;
    repository::hashes::upsert_many(&tx, request.hashes).await?;
    tx.commit().await?;
    Ok(())
}
