use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE UNIQUE INDEX unique_parts_type_and_data_index ON "parts" ("part_type", (md5("data")::uuid));
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX unique_parts_type_and_data_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
