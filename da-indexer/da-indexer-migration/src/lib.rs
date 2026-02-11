pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_create_table;
mod m20240523_095338_eigenda_tables;
mod m20250623_122642_alter_blob_tables_add_data_s3_object_key_column;
mod m20251119_110928_add_eigenda_v2_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20240523_095338_eigenda_tables::Migration),
            Box::new(m20250623_122642_alter_blob_tables_add_data_s3_object_key_column::Migration),
            Box::new(m20251119_110928_add_eigenda_v2_tables::Migration),
        ]
    }
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    let stmnts: Vec<&str> = content.split(';').collect();
    let txn = manager.get_connection().begin().await?;
    for st in stmnts.into_iter() {
        txn.execute(Statement::from_string(
            manager.get_database_backend(),
            st.to_string(),
        ))
        .await
        .map_err(|e| DbErr::Migration(format!("{e}\nQuery: {st}")))?;
    }
    txn.commit().await
}
