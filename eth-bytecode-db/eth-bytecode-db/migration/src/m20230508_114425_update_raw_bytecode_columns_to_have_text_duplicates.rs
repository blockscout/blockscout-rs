use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "sources"
            ADD COLUMN "raw_creation_input_text" text,
            ADD COLUMN "raw_deployed_bytecode_text" text;

            ALTER TABLE "parts"
            ADD COLUMN "data_text" text;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "parts"
            DROP COLUMN "data_text";

            ALTER TABLE "sources"
            DROP COLUMN "raw_creation_input_text",
            DROP COLUMN "raw_deployed_bytecode_text";
        "#;
        crate::from_sql(manager, sql).await
    }
}
