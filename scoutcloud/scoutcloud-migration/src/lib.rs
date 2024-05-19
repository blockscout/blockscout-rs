pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_create_table;
mod m20240208_092748_create_triggers;
mod m20240409_105319_fill_server_specs;
mod m20240415_094154_add_fang;
mod m20240519_102932_add_max_instances;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20240208_092748_create_triggers::Migration),
            Box::new(m20240409_105319_fill_server_specs::Migration),
            Box::new(m20240415_094154_add_fang::Migration),
            Box::new(m20240519_102932_add_max_instances::Migration),
        ]
    }
}

pub async fn from_statements(
    manager: &SchemaManager<'_>,
    statements: &[&str],
) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    for statement in statements {
        txn.execute(Statement::from_string(
            manager.get_database_backend(),
            statement.to_string(),
        ))
        .await
        .map_err(|err| DbErr::Migration(format!("{err}\nQuery: {statement}")))?;
    }
    txn.commit().await
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    let statements: Vec<&str> = content.split(';').collect();
    from_statements(manager, statements.as_slice()).await
}
