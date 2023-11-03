pub mod eth_bytecode_db;
pub mod verifier_alliance_db;

////////////////////////////////////////////////////////////////////////////////////////////

macro_rules! insert_then_select {
    ( $txn:expr, $entity_module:ident, $active_model:expr, $update_on_conflict:expr, [ $( ($column:ident, $value:expr) ),+ $(,)? ] ) => {
        {
            let result: Result<_, sea_orm::DbErr> = $entity_module::Entity::insert($active_model.clone())
                .on_conflict(sea_orm::sea_query::OnConflict::new().do_nothing().to_owned())
                .exec($txn)
                .await;

            // Returns the model and the bool flag showing whether the model was actually inserted.
            match result {
                Ok(res) => {
                    let last_insert_id = res.last_insert_id;
                    let model = $entity_module::Entity::find_by_id(last_insert_id.clone())
                        .one($txn)
                        .await
                        .context(format!("select from \"{}\" by \"id\"", stringify!($entity_module)))?
                        .ok_or(anyhow::anyhow!(
                            "select from \"{}\" by \"id\"={:?} returned no data",
                            stringify!($entity_module),
                            last_insert_id
                        ))?;

                    Ok((model, true))
                }
                Err(sea_orm::DbErr::RecordNotInserted) => {
                    let mut model =
                        $entity_module::Entity::find()
                            $(
                                .filter($entity_module::Column::$column.eq($value))
                            )*
                            .one($txn)
                            .await
                            .context(format!("select from \"{}\" by unique columns", stringify!($entity_module)))?
                            .ok_or(anyhow::anyhow!("select from \"{}\" by unique columns returned no data", stringify!($entity_module)))?;
                    // The active model have not been inserted.
                    // Thus, there were a value already that we need to update.
                    if $update_on_conflict {
                        let mut active_model_to_update = $active_model;
                        for primary_key in <$entity_module::PrimaryKey as sea_orm::Iterable>::iter() {
                            let column = sea_orm::PrimaryKeyToColumn::into_column(primary_key);
                            let value = sea_orm::ModelTrait::get(&model, column);
                            sea_orm::ActiveModelTrait::set(&mut active_model_to_update, column, value);
                        }
                        let updated_model = sea_orm::ActiveModelTrait::update(
                            active_model_to_update, $txn
                        ).await.context(format!("update on conflict in \"{}\"", stringify!($entity_module)))?;

                        if updated_model != model {
                            tracing::warn!(
                                model=?model,
                                updated_model=?updated_model,
                                "the \"{}\" model has been updated",
                                stringify!($entity_module)
                            );
                            model = updated_model;
                        }
                    }

                    Ok((model, false))
                }
                Err(err) => Err(err).context(format!("insert into \"{}\"", stringify!($entity_module))),
            }
        }
    };
}
use insert_then_select;
