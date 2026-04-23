use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            -- General and compiler-specific artifacts (abi, userdoc, devdoc, licenses, ast, etc), encoded as a json.
            ALTER TABLE "sources"
            ADD COLUMN "compilation_artifacts" jsonb;

            -- Info about the creation code (sourcemaps, linkreferences) encoded as a json.
            ALTER TABLE "sources"
            ADD COLUMN "creation_input_artifacts" jsonb;

            -- Info about the runtime code (sourcemaps, linkreferences, immutables) encoded as a json.
            ALTER TABLE "sources"
            ADD COLUMN "deployed_bytecode_artifacts" jsonb;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "sources"
            DROP COLUMN "deployed_bytecode_artifacts";

            ALTER TABLE "sources"
            DROP COLUMN "creation_input_artifacts";

            ALTER TABLE "sources"
            DROP COLUMN "compilation_artifacts";
        "#;
        crate::from_sql(manager, sql).await
    }
}
