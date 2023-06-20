pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, Statement, TransactionTrait};

mod m20220101_000001_init;
mod m20230616_112031_add_block_ranges;
mod m20230619_151130_add_total_supply;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_init::Migration),
            Box::new(m20230616_112031_add_block_ranges::Migration),
            Box::new(m20230619_151130_add_total_supply::Migration),
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
