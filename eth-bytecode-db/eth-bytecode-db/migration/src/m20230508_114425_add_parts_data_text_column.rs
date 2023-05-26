use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "parts"
            ADD COLUMN "data_text" text;

            ALTER TABLE "parts"
                ADD CONSTRAINT valid_hex_parts_data_text CHECK ("data_text" ~ '^[0-9a-f]+$');
            ALTER TABLE "parts"
                ADD CONSTRAINT valid_length_parts_data_text CHECK (length("data_text") % 2 = 0);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "parts"
            DROP COLUMN "data_text";
        "#;
        crate::from_sql(manager, sql).await
    }
}
