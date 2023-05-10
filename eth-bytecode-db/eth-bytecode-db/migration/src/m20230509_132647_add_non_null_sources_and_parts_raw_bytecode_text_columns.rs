use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "sources"
            ALTER COLUMN "raw_creation_input_text" SET NOT NULL;

            ALTER TABLE "sources"
            ALTER COLUMN "raw_deployed_bytecode_text" SET NOT NULL;

            ALTER TABLE "parts"
            ALTER COLUMN "data_text" SET NOT NULL;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "sources"
            ALTER COLUMN "raw_creation_input_text" DROP NOT NULL;

            ALTER TABLE "sources"
            ALTER COLUMN "raw_deployed_bytecode_text" DROP NOT NULL;

            ALTER TABLE "parts"
            ALTER COLUMN "data_text" DROP NOT NULL;
        "#;
        crate::from_sql(manager, sql).await
    }
}
