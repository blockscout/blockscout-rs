use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveValue::Set, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait, Schema,
};
use zetachain_cctx_entity::{sea_orm_active_enums::Kind, watermark};

/// Initialize in-memory DB with watermark for easy testing
pub async fn init_imdb_with_watermark(timestamp: Option<DateTime<Utc>>) -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = Schema::new(DbBackend::Sqlite);
    db.execute(
        db.get_database_backend()
            .build(&schema.create_table_from_entity(watermark::Entity)),
    )
    .await
    .unwrap();
    if let Some(t) = timestamp {
        fill_watermark(&db, t).await;
    }
    db
}

pub async fn fill_watermark(db: &DatabaseConnection, timestamp: DateTime<Utc>) {
    watermark::Entity::insert(watermark::ActiveModel {
        id: Set(1),
        kind: Set(Kind::Historical),
        upper_bound_timestamp: Set(Some(timestamp.naive_utc())),
        pointer: Set("".to_string()),
        processing_status: Set(zetachain_cctx_entity::sea_orm_active_enums::ProcessingStatus::Done),
        created_at: Set(chrono::Utc::now().naive_utc()),
        updated_at: Set(chrono::Utc::now().naive_utc()),
        updated_by: Set("".to_string()),
        retries_number: Set(0),
    })
    .exec(db)
    .await
    .unwrap();
}
