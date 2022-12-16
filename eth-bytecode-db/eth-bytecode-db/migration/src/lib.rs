pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement, TransactionTrait};

mod m20220101_000001_initial;
mod m20221118_182727_rename_types;
mod m20221122_222955_add_indexes;
mod m20221130_231403_add_unique_files_name_and_content_index;
mod m20221201_015147_add_unique_bytecodes_source_id_and_type_index;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial::Migration),
            Box::new(m20221118_182727_rename_types::Migration),
            Box::new(m20221122_222955_add_indexes::Migration),
            Box::new(m20221130_231403_add_unique_files_name_and_content_index::Migration),
            Box::new(m20221201_015147_add_unique_bytecodes_source_id_and_type_index::Migration),
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
        .map_err(|e| DbErr::Migration(format!("{}\nQuery: {}", e, st)))?;
    }
    txn.commit().await
}
