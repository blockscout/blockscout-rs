pub use sea_orm_migration::prelude::*;

mod m20251030_000001_initial;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20251030_000001_initial::Migration)]
    }
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(content).await?;
    Ok(())
}
