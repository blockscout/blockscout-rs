use super::paginate_cursor;
use crate::types::{interop_messages::InteropMessage, ChainId};
use entity::interop_messages::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::{DateTime, Expr},
    sea_query::OnConflict,
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Iterable, PaginatorTrait, QueryFilter,
    QueryTrait,
};

pub async fn upsert_many<C>(db: &C, mut interop_messages: Vec<InteropMessage>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    interop_messages.sort_by(|a, b| (a.init_chain_id, a.nonce).cmp(&(b.init_chain_id, b.nonce)));
    let interop_messages = interop_messages.into_iter().map(ActiveModel::from);

    Entity::insert_many(interop_messages)
        .on_conflict(
            OnConflict::columns([Column::InitChainId, Column::Nonce])
                .update_columns(non_primary_columns())
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .do_nothing()
        .exec_without_returning(db)
        .await?;

    Ok(())
}

pub async fn list<C>(
    db: &C,
    init_chain_id: Option<ChainId>,
    relay_chain_id: Option<ChainId>,
    nonce: Option<i64>,
    page_size: u64,
    page_token: Option<(DateTime, ChainId, i64)>,
) -> Result<(Vec<Model>, Option<(DateTime, ChainId, i64)>), DbErr>
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
        .cursor_by((Column::Timestamp, Column::InitChainId, Column::Nonce));
    c.desc();

    paginate_cursor(db, c, page_size, page_token, |u| {
        (
            u.timestamp.expect("timestamp is not null"),
            u.init_chain_id,
            u.nonce,
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
            Column::InitChainId | Column::Nonce | Column::CreatedAt | Column::UpdatedAt
        )
    })
}
