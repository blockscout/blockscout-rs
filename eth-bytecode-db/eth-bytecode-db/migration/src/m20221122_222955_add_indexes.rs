use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // CREATE EXTENSION pg_trgm;

        // CREATE INDEX bytecode_parts_bytecode_id_index ON bytecode_parts (bytecode_id);
        // CREATE INDEX bytecode_parts_part_id_index ON bytecode_parts (part_id);
        // CREATE INDEX bytecodes_source_id_index ON bytecodes (source_id);
        // CREATE INDEX parts_type_index ON parts(part_type);
        // CREATE INDEX parts_data_index ON parts USING gin ((encode(data, 'hex') || '%') gin_trgm_ops);

        let sql = r#"
        CREATE INDEX bytecode_parts_part_id_index ON bytecode_parts (bytecode_id, part_id);
        CREATE INDEX bytecodes_source_id_index ON bytecodes (source_id);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // DROP INDEX bytecode_parts_bytecode_id_index;
        // DROP INDEX bytecode_parts_part_id_index;
        // DROP INDEX bytecodes_source_id_index;
        // DROP INDEX parts_type_index;
        // DROP INDEX parts_data_index;

        // DROP EXTENSION pg_trgm;

        let sql = r#"
        DROP INDEX bytecode_parts_part_id_index;
        DROP INDEX bytecodes_source_id_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
