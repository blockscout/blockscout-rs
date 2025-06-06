use crate::{error::ServiceError, repository, types::batch_import_request::BatchImportRequest};
use sea_orm::{DatabaseConnection, TransactionTrait};

pub async fn batch_import(
    db: &DatabaseConnection,
    request: BatchImportRequest,
) -> Result<(), ServiceError> {
    request.record_metrics();

    let tx = db.begin().await?;
    repository::addresses::upsert_many(&tx, request.addresses)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert addresses");
        })?;
    repository::block_ranges::upsert_many(&tx, request.block_ranges)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert block ranges");
        })?;
    repository::hashes::upsert_many(&tx, request.hashes)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert hashes");
        })?;
    repository::interop_messages::upsert_many_with_transfers(&tx, request.interop_messages)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert interop messages");
        })?;
    tx.commit().await?;
    Ok(())
}
