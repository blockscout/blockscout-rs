use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX IF NOT EXISTS "idx_parts_data_text_prefix" ON "parts" (LEFT("data_text", 500) text_pattern_ops);
            CREATE INDEX IF NOT EXISTS "idx_parts_data_text_length" ON "parts" (LENGTH("data_text"));
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS "idx_parts_data_text_prefix";
            DROP INDEX IF EXISTS "idx_parts_data_text_length";
        "#;
        crate::from_sql(manager, sql).await
    }
}
