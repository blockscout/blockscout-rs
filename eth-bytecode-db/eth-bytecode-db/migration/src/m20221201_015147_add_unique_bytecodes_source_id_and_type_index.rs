use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE UNIQUE INDEX unique_bytecodes_source_id_and_type_index ON bytecodes (source_id, bytecode_type);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX unique_bytecodes_source_id_and_type_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
