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

            ALTER TABLE "sources"
                ADD CONSTRAINT valid_hex_sources_raw_creation_input_text CHECK (regexp_like("raw_creation_input_text", '^[0-9a-f]+$'));
            ALTER TABLE "sources"
                ADD CONSTRAINT valid_length_sources_raw_creation_input_text CHECK (length("raw_creation_input_text") % 2 = 0);

            ALTER TABLE "sources"
                ADD CONSTRAINT valid_hex_sources_raw_deployed_bytecode_text CHECK (regexp_like("raw_deployed_bytecode_text", '^[0-9a-f]+$'));
            ALTER TABLE "sources"
                ADD CONSTRAINT valid_length_sources_raw_deployed_bytecode_text CHECK (length("raw_deployed_bytecode_text") % 2 = 0);

            ALTER TABLE "parts"
                ADD CONSTRAINT valid_hex_parts_data_text CHECK (regexp_like("data_text", '^[0-9a-f]+$'));
            ALTER TABLE "parts"
                ADD CONSTRAINT valid_length_parts_data_text CHECK (length("data_text") % 2 = 0);
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
