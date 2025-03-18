pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_init;
mod m20230814_105206_drop_zero_timestamp;
mod m20240416_090545_add_updated_at_column;
mod m20240719_133448_add_resolution_column;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_init::Migration),
            Box::new(m20230814_105206_drop_zero_timestamp::Migration),
            Box::new(m20240416_090545_add_updated_at_column::Migration),
            Box::new(m20240719_133448_add_resolution_column::Migration),
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
        .map_err(|e| DbErr::Migration(::std::format!("{e}\nQuery: {st}")))?;
    }
    txn.commit().await
}
