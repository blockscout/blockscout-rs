pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20230426_170496_create_functions;
mod m20230426_170508_create_language_enum;
mod m20230426_170520_create_status_enum;
mod m20230426_170541_create_contract_addresses_table;
mod m20230426_170553_create_contract_details_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20230426_170496_create_functions::Migration),
            Box::new(m20230426_170508_create_language_enum::Migration),
            Box::new(m20230426_170520_create_status_enum::Migration),
            Box::new(m20230426_170541_create_contract_addresses_table::Migration),
            Box::new(m20230426_170553_create_contract_details_table::Migration),
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
