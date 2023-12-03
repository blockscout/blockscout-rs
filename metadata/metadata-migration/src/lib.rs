pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_public_tags;
mod m20231201_101921_notes;
mod m20231201_210832_reputation;
mod m20231201_211008_triggers;

pub struct Migrator;

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    exec_stmts(manager, content.split(';')).await
}

pub async fn exec_stmts(
    manager: &SchemaManager<'_>,
    stmts: impl IntoIterator<Item = &str>,
) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    for st in stmts {
        println!("Executing: {}", st);
        txn.execute(Statement::from_string(
            manager.get_database_backend(),
            st.to_string(),
        ))
        .await
        .map_err(|e| DbErr::Migration(format!("{e}\nQuery: {st}")))?;
    }
    txn.commit().await
}

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_public_tags::Migration),
            Box::new(m20231201_101921_notes::Migration),
            Box::new(m20231201_210832_reputation::Migration),
            Box::new(m20231201_211008_triggers::Migration),
        ]
    }
}
