use crate::{
    error::ServiceError,
    proto, repository,
    services::channel::{LatestBlockUpdateMessage, NEW_BLOCKS_TOPIC, NEW_INTEROP_MESSAGES_TOPIC},
    types::{batch_import_request::BatchImportRequest, interop_messages::InteropMessage},
};
use actix_phoenix_channel::ChannelBroadcaster;
use sea_orm::{DatabaseConnection, TransactionTrait};

pub async fn batch_import(
    db: &DatabaseConnection,
    request: BatchImportRequest,
    channel: ChannelBroadcaster,
) -> Result<(), ServiceError> {
    request.record_metrics();

    let tx = db.begin().await?;
    repository::addresses::upsert_many(&tx, request.addresses)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert addresses");
        })?;
    let block_ranges = repository::block_ranges::upsert_many(&tx, request.block_ranges)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert block ranges");
        })?;
    repository::hashes::upsert_many(&tx, request.hashes)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert hashes");
        })?;
    let messages_with_transfers =
        repository::interop_messages::upsert_many_with_transfers(&tx, request.interop_messages)
            .await
            .inspect_err(|e| {
                tracing::error!(error = ?e, "failed to upsert interop messages");
            })?;
    repository::address_coin_balances::upsert_many(&tx, request.address_coin_balances)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert address coin balances");
        })?;
    repository::address_token_balances::upsert_many(&tx, request.address_token_balances)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert address token balances");
        })?;
    repository::tokens::upsert_many(&tx, request.tokens)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert tokens");
        })?;
    if let Some(counters) = request.counters {
        if let Some(global) = counters.global {
            repository::counters::upsert_chain_counters(&tx, global)
                .await
                .inspect_err(|e| {
                    tracing::error!(error = ?e, "failed to upsert chain counters");
                })?;
        }
    }

    tx.commit().await?;

    let interop_messages = messages_with_transfers
        .into_iter()
        .filter(|(m, _)| m.init_transaction_hash.is_some())
        .filter_map(|m| InteropMessage::try_from(m).ok())
        .map(proto::InteropMessage::from)
        .collect::<Vec<_>>();
    if !interop_messages.is_empty() {
        channel.broadcast((NEW_INTEROP_MESSAGES_TOPIC, "new_messages", interop_messages));
    }

    let block_ranges = block_ranges
        .into_iter()
        .map(|m| LatestBlockUpdateMessage {
            chain_id: m.chain_id,
            block_number: m.max_block_number,
        })
        .collect::<Vec<_>>();
    if !block_ranges.is_empty() {
        channel.broadcast((NEW_BLOCKS_TOPIC, "new_blocks", block_ranges));
    }

    Ok(())
}
