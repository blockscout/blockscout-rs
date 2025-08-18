use crate::{
    error::ServiceError,
    proto, repository,
    services::channel::{LatestBlockUpdateMessage, NEW_BLOCKS_TOPIC, NEW_INTEROP_MESSAGES_TOPIC},
    types::{
        batch_import_request::BatchImportRequest,
        interop_messages::InteropMessage,
        tokens::{TokenType, TokenUpdate, UpdateTokenType},
    },
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

    let token_type_updates = prepare_erc_7802_token_updates(&messages_with_transfers);
    let mut token_updates = request.tokens;
    token_updates.extend(token_type_updates);

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
    repository::tokens::upsert_many(&tx, token_updates)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert tokens");
        })?;
    if let Some(counters) = request.counters
        && let Some(global) = counters.global
    {
        repository::counters::upsert_chain_counters(&tx, global)
            .await
            .inspect_err(|e| {
                tracing::error!(error = ?e, "failed to upsert chain counters");
            })?;
    }

    tx.commit().await?;

    let interop_messages = messages_with_transfers
        .into_iter()
        .filter(|m| m.init_transaction_hash.is_some())
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

fn prepare_erc_7802_token_updates(messages_with_transfers: &[InteropMessage]) -> Vec<TokenUpdate> {
    messages_with_transfers
        .iter()
        .filter_map(|message| {
            let address_hash = message
                .transfer
                .as_ref()?
                .token_address_hash
                .as_ref()?
                .to_vec();
            Some([
                UpdateTokenType {
                    chain_id: message.init_chain_id,
                    address_hash: address_hash.clone(),
                    token_type: TokenType::Erc7802,
                },
                UpdateTokenType {
                    chain_id: message.relay_chain_id,
                    address_hash: address_hash.clone(),
                    token_type: TokenType::Erc7802,
                },
            ])
        })
        .flatten()
        .map(|t| TokenUpdate {
            r#type: Some(t),
            ..Default::default()
        })
        .collect()
}
