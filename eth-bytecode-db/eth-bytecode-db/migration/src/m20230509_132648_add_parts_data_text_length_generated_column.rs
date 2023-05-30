use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "parts"
            ADD COLUMN "data_text_length" INT GENERATED ALWAYS AS (length("data_text")) STORED;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "parts"
            DROP COLUMN "data_text_length";
        "#;
        crate::from_sql(manager, sql).await
    }
}
