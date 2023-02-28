use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;
// (md5(row(col1, col2, col3)::uuid))
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE UNIQUE INDEX unique_source_index ON "sources"
            ("compiler_version", md5("compiler_settings"::text), "file_name", "contract_name", "file_ids_hash");
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX unique_source_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
