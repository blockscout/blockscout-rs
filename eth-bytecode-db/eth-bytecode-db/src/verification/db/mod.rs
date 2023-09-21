pub mod eth_bytecode_db;
pub mod verifier_alliance_db;

////////////////////////////////////////////////////////////////////////////////////////////

macro_rules! insert_then_select {
    ( $txn:expr, $entity_module:ident, $active_model:expr, [ $( ($column:ident, $value:expr) ),+ $(,)? ] ) => {
        {
            let result: Result<_, sea_orm::DbErr> = $entity_module::Entity::insert($active_model)
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
                    let model = $entity_module::Entity::find()
                        $(
                            .filter($entity_module::Column::$column.eq($value))
                        )*
                        .one($txn)
                        .await
                        .context(format!("select from \"{}\" by unique columns", stringify!($entity_module)))?
                        .ok_or(anyhow::anyhow!("select from \"{}\" by unique columns returned no data", stringify!($entity_module)))?;

                    Ok((model, false))
                }
                Err(err) => Err(err).context(format!("insert into \"{}\"", stringify!($entity_module))),
            }
        }
    };
}
use insert_then_select;
