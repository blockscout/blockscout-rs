use super::{interop_message_transfers, macros::update_if_not_null, paginate_cursor};
use crate::types::{
    ChainId,
    interop_message_transfers::InteropMessageTransfer,
    interop_messages::{InteropMessage, MessageDirection},
};
use alloy_primitives::{Address as AddressAlloy, TxHash};
use entity::{
    interop_messages::{ActiveModel, Column, Entity, Model},
    interop_messages_transfers,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, IdenStatic, PaginatorTrait, QueryFilter,
    QueryTrait, TransactionError, TransactionTrait,
    prelude::{DateTime, Expr},
    sea_query::OnConflict,
};
use std::collections::HashMap;

pub async fn upsert_many_with_transfers<C>(
    db: &C,
    mut interop_messages: Vec<(InteropMessage, Option<InteropMessageTransfer>)>,
) -> Result<Vec<(Model, Option<interop_messages_transfers::Model>)>, DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    // Return early because we don't use `.do_nothing()` for messages batch insert
    // to avoid pattern matching on `TryInsertResult`
    if interop_messages.is_empty() {
        return Ok(vec![]);
    }

    interop_messages
        .sort_by(|(a, _), (b, _)| (a.init_chain_id, a.nonce).cmp(&(b.init_chain_id, b.nonce)));

    let (interop_messages, transfers): (Vec<_>, Vec<_>) = interop_messages
        .into_iter()
        .map(|(m, t)| (ActiveModel::from(m), t))
        .unzip();

    db.transaction(|tx| {
        Box::pin(async move {
            let messages = Entity::insert_many(interop_messages)
                .on_conflict(
                    OnConflict::columns([Column::Nonce, Column::InitChainId])
                        .values([
                            update_if_not_null!(Column::SenderAddressHash),
                            update_if_not_null!(Column::TargetAddressHash),
                            update_if_not_null!(Column::InitTransactionHash),
                            update_if_not_null!(Column::Timestamp),
                            update_if_not_null!(Column::RelayTransactionHash),
                            update_if_not_null!(Column::Payload),
                            update_if_not_null!(Column::Failed),
                        ])
                        .update_column(Column::RelayChainId)
                        .value(Column::UpdatedAt, Expr::current_timestamp())
                        .to_owned(),
                )
                .exec_with_returning_many(tx)
                .await?;

            // Set interop_message_id for each corresponding transfer after all messages are inserted
            let transfers = messages
                .iter()
                .zip(transfers)
                .filter_map(|(m, t)| t.map(|t| (t, m.id)))
                .collect();

            let transfers = interop_message_transfers::upsert_many(tx, transfers).await?;

            let mut id_to_transfer = transfers
                .into_iter()
                .map(|t| (t.interop_message_id, t))
                .collect::<HashMap<_, _>>();

            Ok(messages
                .into_iter()
                .map(|m| {
                    let t = id_to_transfer.remove(&m.id);
                    (m, t)
                })
                .collect())
        })
    })
    .await
    .map_err(|err| match err {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn list<C>(
    db: &C,
    init_chain_id: Option<ChainId>,
    relay_chain_id: Option<ChainId>,
    address: Option<AddressAlloy>,
    direction: Option<MessageDirection>,
    nonce: Option<i64>,
    cluster_chain_ids: Option<Vec<ChainId>>,
    page_size: u64,
    page_token: Option<(DateTime, TxHash)>,
) -> Result<
    (
        Vec<(Model, Option<interop_messages_transfers::Model>)>,
        Option<(DateTime, TxHash)>,
    ),
    DbErr,
>
where
    C: ConnectionTrait,
{
    let mut c = Entity::find()
        .filter(Column::InitTransactionHash.is_not_null())
        .apply_if(cluster_chain_ids, |q, cluster_chain_ids| {
            q.filter(
                Column::InitChainId
                    .is_in(cluster_chain_ids.clone())
                    .and(Column::RelayChainId.is_in(cluster_chain_ids)),
            )
        })
        .apply_if(init_chain_id, |q, init_chain_id| {
            q.filter(Column::InitChainId.eq(init_chain_id))
        })
        .apply_if(relay_chain_id, |q, relay_chain_id| {
            q.filter(Column::RelayChainId.eq(relay_chain_id))
        })
        .apply_if(address, |q, address| {
            let address = address.as_slice();
            let sender_cond = Column::SenderAddressHash.eq(address);
            let target_cond = Column::TargetAddressHash.eq(address);
            match direction {
                Some(MessageDirection::From) => q.filter(sender_cond),
                Some(MessageDirection::To) => q.filter(target_cond),
                None => q.filter(sender_cond.or(target_cond)),
            }
        })
        .apply_if(nonce, |q, nonce| q.filter(Column::Nonce.eq(nonce)))
        .find_also_related(interop_messages_transfers::Entity)
        .cursor_by((Column::Timestamp, Column::InitTransactionHash));
    c.desc();

    let page_token = page_token.map(|(t, h)| (t, h.to_vec()));

    paginate_cursor(db, c, page_size, page_token, |(u, _)| {
        let init_transaction_hash = u
            .init_transaction_hash
            .as_ref()
            .expect("init_transaction_hash is not null")
            .as_slice();
        (
            u.timestamp.expect("timestamp is not null"),
            TxHash::try_from(init_transaction_hash).expect("init_transaction_hash is valid"),
        )
    })
    .await
}

pub async fn count<C>(
    db: &C,
    chain_id: ChainId,
    cluster_chain_ids: Option<Vec<ChainId>>,
) -> Result<u64, DbErr>
where
    C: ConnectionTrait,
{
    Entity::find()
        .filter(Column::InitTransactionHash.is_not_null())
        .apply_if(cluster_chain_ids, |q, cluster_chain_ids| {
            q.filter(
                Column::InitChainId
                    .is_in(cluster_chain_ids.clone())
                    .and(Column::RelayChainId.is_in(cluster_chain_ids)),
            )
        })
        .filter(
            Column::InitChainId
                .eq(chain_id)
                .or(Column::RelayChainId.eq(chain_id)),
        )
        .count(db)
        .await
}
