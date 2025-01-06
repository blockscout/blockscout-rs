use anyhow::Context;
use sea_orm::{
    sea_query::OnConflict, ActiveModelBehavior, ActiveModelTrait, ColumnTrait, ConnectionTrait,
    DbErr, EntityTrait, IntoActiveModel, ModelTrait, PrimaryKeyToColumn, QueryFilter,
};

pub async fn insert_then_select<C, Entity, ActiveModel>(
    txn: &C,
    entity: Entity,
    active_model: ActiveModel,
    unique_columns: impl IntoIterator<Item = (Entity::Column, sea_orm::Value)>,
) -> Result<(Entity::Model, bool), anyhow::Error>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: ActiveModelTrait<Entity = Entity> + ActiveModelBehavior + Send,
    <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
{
    insert_then_select_internal(txn, entity, active_model, unique_columns, false).await
}

async fn insert_then_select_internal<C, Entity, ActiveModel>(
    txn: &C,
    entity: Entity,
    active_model: ActiveModel,
    unique_columns: impl IntoIterator<Item = (Entity::Column, sea_orm::Value)>,
    update_on_conflict: bool,
) -> Result<(Entity::Model, bool), anyhow::Error>
where
    C: ConnectionTrait,
    Entity: EntityTrait,
    ActiveModel: ActiveModelTrait<Entity = Entity> + ActiveModelBehavior + Send,
    <Entity as EntityTrait>::Model: IntoActiveModel<ActiveModel>,
{
    let entity_table_name = entity.table_name();

    let result: Result<_, DbErr> = Entity::insert(active_model.clone())
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec(txn)
        .await;

    // Returns the model and the bool flag showing whether the model was actually inserted.
    match result {
        Ok(res) => {
            let last_insert_id = res.last_insert_id;
            let id_debug_str = format!("{last_insert_id:?}");
            let model = Entity::find_by_id(last_insert_id)
                .one(txn)
                .await
                .context(format!("select from \"{entity_table_name}\" by \"id\""))?
                .ok_or(anyhow::anyhow!(
                    "select from \"{entity_table_name}\" by \"id\"={id_debug_str} returned no data"
                ))?;

            Ok((model, true))
        }
        Err(DbErr::RecordNotInserted) => {
            let mut query = Entity::find();
            for (column, value) in unique_columns {
                query = query.filter(column.eq(value));
            }
            let mut model = query
                .one(txn)
                .await
                .context(format!(
                    "select from \"{entity_table_name}\" by unique columns"
                ))?
                .ok_or(anyhow::anyhow!(
                    "select from \"{entity_table_name}\" by unique columns returned no data"
                ))?;

            // The active model have not been inserted.
            // Thus, there were a value already that we need to update.
            if update_on_conflict {
                let mut active_model_to_update = active_model;
                for primary_key in <Entity::PrimaryKey as sea_orm::Iterable>::iter() {
                    let column = PrimaryKeyToColumn::into_column(primary_key);
                    let value = ModelTrait::get(&model, column);
                    ActiveModelTrait::set(&mut active_model_to_update, column, value);
                }
                model = active_model_to_update
                    .update(txn)
                    .await
                    .context(format!("update on conflict in \"{entity_table_name}\""))?;
            }

            Ok((model, false))
        }
        Err(err) => Err(err).context(format!("insert into \"{entity_table_name}\"")),
    }
}
