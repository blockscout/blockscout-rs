use crate::types::addresses::Address;
use entity::addresses::{ActiveModel, Column, Entity, Model};
use sea_orm::{
    prelude::Expr, sea_query::OnConflict, ActiveValue::NotSet, ConnectionTrait, DbErr, EntityTrait,
    Iterable,
};

pub async fn upsert_many<C>(db: &C, addresses: Vec<Address>) -> Result<(), DbErr>
where
    C: ConnectionTrait,
{
    if addresses.is_empty() {
        return Ok(());
    }

    let addresses = addresses.into_iter().map(|address| {
        let model: Model = address.into();
        let mut active: ActiveModel = model.into();
        active.created_at = NotSet;
        active.updated_at = NotSet;
        active
    });

    Entity::insert_many(addresses)
        .on_conflict(
            OnConflict::columns([Column::Hash, Column::ChainId])
                .update_columns(non_primary_columns())
                .value(Column::UpdatedAt, Expr::current_timestamp())
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}

fn non_primary_columns() -> impl Iterator<Item = Column> {
    Column::iter().filter(|col| {
        !matches!(
            col,
            Column::Hash | Column::ChainId | Column::CreatedAt | Column::UpdatedAt
        )
    })
}
