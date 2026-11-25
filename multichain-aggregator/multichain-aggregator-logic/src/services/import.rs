// SPDX-License-Identifier: LicenseRef-Blockscout

use std::collections::HashSet;

use crate::{
    error::ServiceError,
    repository, services,
    services::channel::{LatestBlockUpdateMessage, NEW_BLOCKS_TOPIC},
    types::{
        address_token_balances::AddressTokenBalance, batch_import_request::BatchImportRequest,
        poor_reputation_tokens::PoorReputationToken,
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

    repository::tokens::upsert_many(&tx, request.tokens)
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
