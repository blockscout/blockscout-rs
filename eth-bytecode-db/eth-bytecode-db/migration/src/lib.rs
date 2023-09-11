pub use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{Statement, TransactionTrait};

mod m20220101_000001_initial_tables;
mod m20221122_222955_add_indexes;
mod m20221130_231403_add_unique_files_name_and_content_index;
mod m20221201_015147_add_unique_bytecodes_source_id_and_type_index;
mod m20230222_194726_add_unique_parts_type_and_data_index;
mod m20230227_014110_add_unique_source_index;
mod m20230316_020341_verified_contracts_add_chain_id_contract_address_columns;
mod m20230508_114425_add_parts_data_text_column;
mod m20230509_103951_add_trigger_parts_convert_data_to_text_value;
mod m20230509_123604_duplicate_parts_existing_data_to_text_column;
mod m20230509_132647_add_not_null_constraint_parts_data_text_column;
mod m20230510_151046_add_search_speedup_indexes_on_parts;
mod m20230531_191321_update_parts_data_text_prefix_index_to_150_chars;
mod m20230911_103441_update_sources_add_verification_artifact_columns;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_initial_tables::Migration),
            Box::new(m20221122_222955_add_indexes::Migration),
            Box::new(m20221130_231403_add_unique_files_name_and_content_index::Migration),
            Box::new(m20221201_015147_add_unique_bytecodes_source_id_and_type_index::Migration),
            Box::new(m20230222_194726_add_unique_parts_type_and_data_index::Migration),
            Box::new(m20230227_014110_add_unique_source_index::Migration),
            Box::new(m20230316_020341_verified_contracts_add_chain_id_contract_address_columns::Migration),
            Box::new(m20230508_114425_add_parts_data_text_column::Migration),
            Box::new(m20230509_103951_add_trigger_parts_convert_data_to_text_value::Migration),
            Box::new(m20230509_123604_duplicate_parts_existing_data_to_text_column::Migration),
            Box::new(m20230509_132647_add_not_null_constraint_parts_data_text_column::Migration),
            Box::new(m20230510_151046_add_search_speedup_indexes_on_parts::Migration),
            Box::new(m20230531_191321_update_parts_data_text_prefix_index_to_150_chars::Migration),
            Box::new(m20230911_103441_update_sources_add_verification_artifact_columns::Migration),
        ]
    }
}

pub async fn from_sql(manager: &SchemaManager<'_>, content: &str) -> Result<(), DbErr> {
    exec_stmts(manager, content.split(';')).await
}

pub async fn exec_stmts(
    manager: &SchemaManager<'_>,
    stmts: impl IntoIterator<Item = &str>,
) -> Result<(), DbErr> {
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
