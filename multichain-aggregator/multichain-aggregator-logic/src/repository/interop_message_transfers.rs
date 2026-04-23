use crate::types::{ChainId, interop_message_transfers::InteropMessageTransfer};
use entity::{
    interop_messages,
    interop_messages_transfers::{ActiveModel, Column, Entity, Model},
};
use sea_orm::{
    ActiveValue::Set,
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, Iterable,
    prelude::Expr,
    sea_query::{OnConflict, Query},
};

pub async fn upsert_many<C>(
    db: &C,
    transfers: Vec<(InteropMessageTransfer, i64)>,
) -> Result<Vec<Model>, DbErr>
where
    C: ConnectionTrait,
{
    if transfers.is_empty() {
        return Ok(vec![]);
    }

    let transfers = transfers.into_iter().map(|(transfer, id)| {
        let mut t = ActiveModel::from(transfer);
        t.interop_message_id = Set(id);
        t
    });

    let models = Entity::insert_many(transfers)
        .on_conflict(
            OnConflict::columns([Column::InteropMessageId])
                .do_nothing()
                .update_columns(non_primary_columns())
                .to_owned(),
        )
        .exec_with_returning_many(db)
        .await?;

    Ok(models)
}

fn non_primary_columns() -> impl Iterator<Item = Column> {
    Column::iter().filter(|col| !matches!(col, Column::InteropMessageId))
}

pub async fn check_if_interop_message_transfers_at_address<C>(
    db: &C,
    address: alloy_primitives::Address,
    cluster_chain_ids: Vec<ChainId>,
) -> Result<bool, DbErr>
where
    C: ConnectionTrait,
{
    let query = Query::select()
        .expr(Expr::exists(
            Query::select()
                .column(Column::InteropMessageId)
                .from(Entity)
                .inner_join(
                    interop_messages::Entity,
                    Expr::col(Column::InteropMessageId).eq(Expr::col(interop_messages::Column::Id)),
                )
                .and_where(
                    Column::ToAddressHash
                        .eq(address.as_slice())
                        .or(Column::FromAddressHash.eq(address.as_slice())),
                )
                .and_where(
                    interop_messages::Column::InitChainId
                        .is_in(cluster_chain_ids.clone())
                        .and(interop_messages::Column::RelayChainId.is_in(cluster_chain_ids)),
                )
                .to_owned(),
        ))
        .to_owned();

    db.query_one(db.get_database_backend().build(&query))
        .await?
        .expect("expr should be present")
        .try_get_by_index(0)
}
