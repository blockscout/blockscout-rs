use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX IF NOT EXISTS "idx_sources_raw_creation_input_text_prefix" ON "sources" USING btree (LEFT("raw_creation_input_text", 500) text_pattern_ops);
            CREATE INDEX IF NOT EXISTS "idx_sources_rraw_deployed_bytecode_text_prefix" ON "sources" USING btree (LEFT("raw_deployed_bytecode_text", 500) text_pattern_ops);
            CREATE INDEX IF NOT EXISTS "idx_parts_data_text_prefix" ON "parts" USING btree (LEFT("data_text", 500) text_pattern_ops);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
        DROP INDEX IF EXISTS "idx_sources_raw_creation_input_text_prefix";
        DROP INDEX IF EXISTS "idx_sources_rraw_deployed_bytecode_text_prefix";
        DROP INDEX IF EXISTS "idx_parts_data_text_prefix";
        "#;
        crate::from_sql(manager, sql).await
    }
}
