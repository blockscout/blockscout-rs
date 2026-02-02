use std::collections::HashSet;

use crate::{
    error::ServiceError,
    proto, repository, services,
    services::channel::{LatestBlockUpdateMessage, NEW_BLOCKS_TOPIC, NEW_INTEROP_MESSAGES_TOPIC},
    types::{
        address_token_balances::AddressTokenBalance,
        batch_import_request::BatchImportRequest,
        interop_messages::InteropMessage,
        poor_reputation_tokens::PoorReputationToken,
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

    // Temporarily save address_coin_balances for backward compatibility
    // until address_coin_balances is fully deprecated
    repository::address_coin_balances::upsert_many(&tx, request.address_coin_balances.clone())
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert address coin balances");
        })?;
    repository::address_token_balances::upsert_many(&tx, request.address_token_balances)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert address token balances");
        })?;

    let native_token_balances = request
        .address_coin_balances
        .into_iter()
        .map(AddressTokenBalance::from)
        .collect();
    repository::address_token_balances::upsert_many(&tx, native_token_balances)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert native coin token balances");
        })?;

    repository::tokens::upsert_many(&tx, token_updates)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert tokens");
        })?;
    repository::counters::upsert_many(&tx, request.counters)
        .await
        .inspect_err(|e| {
            tracing::error!(error = ?e, "failed to upsert chain counters");
        })?;

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

pub async fn import_poor_reputation_tokens(
    db: &DatabaseConnection,
    tokens: Vec<PoorReputationToken>,
) -> Result<(), ServiceError> {
    let valid_chain_ids = services::chains::list_repo_chains_cached(db, false)
        .await?
        .into_iter()
        .map(|c| c.id)
        .collect::<HashSet<_>>();
    let tokens = tokens
        .into_iter()
        .filter(|t| valid_chain_ids.contains(&t.chain_id))
        .collect();

    repository::poor_reputation_tokens::upsert_many(db, tokens)
        .await
        .inspect_err(|err| {
            tracing::error!(error = ?err, "failed to import poor reputation tokens");
        })?;
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
