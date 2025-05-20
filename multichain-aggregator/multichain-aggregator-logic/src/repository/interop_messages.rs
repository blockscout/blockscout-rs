use super::{interop_message_transfers, paginate_cursor};
use crate::types::{
    interop_message_transfers::InteropMessageTransfer, interop_messages::InteropMessage, ChainId,
};
use alloy_primitives::TxHash;
use entity::interop_messages::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::{DateTime, Expr},
    sea_query::OnConflict,
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Iterable, PaginatorTrait, QueryFilter,
    QueryTrait, TransactionError, TransactionTrait,
};

pub async fn upsert_many_with_transfers<C>(
    db: &C,
    mut interop_messages: Vec<(InteropMessage, Option<InteropMessageTransfer>)>,
) -> Result<(), DbErr>
where
    C: ConnectionTrait + TransactionTrait,
{
    // Return early because we don't use `.do_nothing()` for messages batch insert
    // to avoid pattern matching on `TryInsertResult`
    if interop_messages.is_empty() {
        return Ok(());
    }

    interop_messages
        .sort_by(|(a, _), (b, _)| (a.init_chain_id, a.nonce).cmp(&(b.init_chain_id, b.nonce)));

    let (interop_messages, transfers): (Vec<_>, Vec<_>) = interop_messages
        .into_iter()
        .map(|(m, t)| (ActiveModel::from(m), t))
        .unzip();

    db.transaction(|tx| {
        Box::pin(async move {
            let message_ids = Entity::insert_many(interop_messages)
                .on_conflict(
                    OnConflict::columns([Column::InitChainId, Column::Nonce])
                        .update_columns(non_primary_columns())
                        .value(Column::UpdatedAt, Expr::current_timestamp())
                        .to_owned(),
                )
                .exec_with_returning_keys(tx)
                .await?;

            // Set interop_message_id for each corresponding transfer after all messages are inserted
            let transfers = message_ids
                .into_iter()
                .zip(transfers)
                .filter_map(|(id, transfer)| transfer.map(|t| (t, id)))
                .collect();

            interop_message_transfers::upsert_many(tx, transfers).await?;

            Ok(())
        })
    })
    .await
    .map_err(|err| match err {
        TransactionError::Connection(e) => e,
        TransactionError::Transaction(e) => e,
    })?;

    Ok(())
}

pub async fn list<C>(
    db: &C,
    init_chain_id: Option<ChainId>,
    relay_chain_id: Option<ChainId>,
    nonce: Option<i64>,
    page_size: u64,
    page_token: Option<(DateTime, TxHash)>,
) -> Result<(Vec<Model>, Option<(DateTime, TxHash)>), DbErr>
where
    C: ConnectionTrait,
{
    let mut c = Entity::find()
        .filter(Column::InitTransactionHash.is_not_null())
        .apply_if(init_chain_id, |q, init_chain_id| {
            q.filter(Column::InitChainId.eq(init_chain_id))
        })
        .apply_if(relay_chain_id, |q, relay_chain_id| {
            q.filter(Column::RelayChainId.eq(relay_chain_id))
        })
        .apply_if(nonce, |q, nonce| q.filter(Column::Nonce.eq(nonce)))
        .cursor_by((Column::Timestamp, Column::InitTransactionHash));
    c.desc();

    let page_token = page_token.map(|(t, h)| (t, h.to_vec()));

    paginate_cursor(db, c, page_size, page_token, |u| {
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

pub async fn count<C>(db: &C, chain_id: ChainId) -> Result<u64, DbErr>
where
    C: ConnectionTrait,
{
    Entity::find()
        .filter(Column::InitTransactionHash.is_not_null())
        .filter(
            Column::InitChainId
                .eq(chain_id)
                .or(Column::RelayChainId.eq(chain_id)),
        )
        .count(db)
        .await
}

fn non_primary_columns() -> impl Iterator<Item = Column> {
    Column::iter().filter(|col| {
        !matches!(
            col,
            Column::Id
                | Column::InitChainId
                | Column::Nonce
                | Column::CreatedAt
                | Column::UpdatedAt
        )
    })
}
