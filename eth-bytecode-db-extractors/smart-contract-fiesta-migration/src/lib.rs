pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20230426_170508_create_verification_method_enum;
mod m20230426_170520_create_status_enum;
mod m20230426_170541_create_contract_addresses_table;
mod m20230426_170553_create_solidity_singles_table;
mod m20230426_170602_create_solidity_multiples_table;
mod m20230426_170607_create_solidity_standard_table;
mod m20230426_170614_create_vyper_singles_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230426_170508_create_verification_method_enum::Migration),
            Box::new(m20230426_170520_create_status_enum::Migration),
            Box::new(m20230426_170541_create_contract_addresses_table::Migration),
            Box::new(m20230426_170553_create_solidity_singles_table::Migration),
            Box::new(m20230426_170602_create_solidity_multiples_table::Migration),
            Box::new(m20230426_170607_create_solidity_standard_table::Migration),
            Box::new(m20230426_170614_create_vyper_singles_table::Migration),
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
