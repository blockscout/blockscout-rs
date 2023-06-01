use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS "idx_parts_data_text_prefix";
            CREATE INDEX "idx_parts_data_text_prefix" ON "parts" (LEFT("data_text", 150) text_pattern_ops);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS "idx_parts_data_text_prefix";
            CREATE INDEX "idx_parts_data_text_prefix" ON "parts" (LEFT("data_text", 500) text_pattern_ops);
        "#;
        crate::from_sql(manager, sql).await
    }
}
