use entity::kv_storage;
use migration::DbErr;
use sea_orm::{sea_query, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

pub async fn get_value(db: &DatabaseConnection, key: &str) -> Result<Option<String>, DbErr> {
    let value = kv_storage::Entity::find()
        .filter(kv_storage::Column::Key.eq(key))
        .one(db)
        .await?;
    Ok(value.map(|model| model.value))
}

pub async fn set_value(db: &DatabaseConnection, key: &str, value: &str) -> Result<(), DbErr> {
    let item = kv_storage::ActiveModel {
        key: Set(key.to_string()),
        value: Set(value.to_string()),
    };

    kv_storage::Entity::insert(item)
        .on_conflict(
            sea_query::OnConflict::column(kv_storage::Column::Key)
                .update_column(kv_storage::Column::Value)
                .to_owned(),
        )
        .exec(db)
        .await?;

    Ok(())
}
