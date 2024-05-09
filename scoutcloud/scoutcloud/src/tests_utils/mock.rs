use sea_orm::{ConnectionTrait, DatabaseConnection};

pub async fn insert_default_data(db: &DatabaseConnection) -> Result<(), anyhow::Error> {
    db.execute_unprepared(include_str!("data/default.sql"))
        .await?;
    Ok(())
}
