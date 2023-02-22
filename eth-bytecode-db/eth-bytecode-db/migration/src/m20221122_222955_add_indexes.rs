use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX bytecode_parts_part_id_index ON bytecode_parts (bytecode_id, part_id);
            CREATE INDEX bytecodes_source_id_index ON bytecodes (source_id);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX bytecode_parts_part_id_index;
            DROP INDEX bytecodes_source_id_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
