pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};
mod m20220101_000001_initial_tables;
mod m20231117_093738_add_indexes;
mod m20240206_150422_add_entrypoint_version;
mod m20240717_111524_add_transaction_hash_index;
mod m20250326_080441_entrypoint_v08;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial_tables::Migration),
            Box::new(m20231117_093738_add_indexes::Migration),
            Box::new(m20240206_150422_add_entrypoint_version::Migration),
            Box::new(m20240717_111524_add_transaction_hash_index::Migration),
            Box::new(m20250326_080441_entrypoint_v08::Migration),
        ]
    }
    fn migration_table_name() -> DynIden {
        Alias::new("user_ops_indexer_migrations").into_iden()
    }
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    let stmts: Vec<&str> = content.split(';').collect();
    let txn = manager.get_connection().begin().await?;
    for st in stmts {
        txn.execute(Statement::from_string(
            manager.get_database_backend(),
            st.to_string(),
        ))
        .await
        .map_err(|e| DbErr::Migration(format!("{e}\nQuery: {st}")))?;
    }
    txn.commit().await
}
