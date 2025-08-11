pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_initial_tables;
mod m20250130_084023_add_chains_name;
mod m20250427_051405_add_interop_messages;
mod m20250602_105925_remove_interop_message_chain_id_ref;
mod m20250604_091215_add_token_and_coin_balances;
mod m20250611_103754_add_counters;
mod m20250721_093013_make_token_balance_nullable;
mod m20250723_084105_add_tokens;
mod m20250729_111157_change_block_ranges_block_number_to_bigint;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial_tables::Migration),
            Box::new(m20250130_084023_add_chains_name::Migration),
            Box::new(m20250427_051405_add_interop_messages::Migration),
            Box::new(m20250602_105925_remove_interop_message_chain_id_ref::Migration),
            Box::new(m20250604_091215_add_token_and_coin_balances::Migration),
            Box::new(m20250611_103754_add_counters::Migration),
            Box::new(m20250721_093013_make_token_balance_nullable::Migration),
            Box::new(m20250723_084105_add_tokens::Migration),
            Box::new(m20250729_111157_change_block_ranges_block_number_to_bigint::Migration),
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
            String::from(*statement),
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
