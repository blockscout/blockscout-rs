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
mod m20250822_103440_add_tokens_indexes;
mod m20250904_113635_add_token_holders_index;
mod m20260119_122518_add_token_updates_index;
mod m20260121_183201_add_zrc2_token_type;
mod m20260122_155207_add_native_token_type;
mod m20260201_195943_add_poor_reputation_tokens;

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
            Box::new(m20250822_103440_add_tokens_indexes::Migration),
            Box::new(m20250904_113635_add_token_holders_index::Migration),
            Box::new(m20260119_122518_add_token_updates_index::Migration),
            Box::new(m20260121_183201_add_zrc2_token_type::Migration),
            Box::new(m20260122_155207_add_native_token_type::Migration),
            Box::new(m20260201_195943_add_poor_reputation_tokens::Migration),
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
