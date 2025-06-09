use crate::types::interop_message_transfers::InteropMessageTransfer;
use entity::interop_messages_transfers::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ConnectionTrait, DbErr, EntityTrait, Iterable,
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
