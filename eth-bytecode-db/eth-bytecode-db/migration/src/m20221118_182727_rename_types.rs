use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
        ALTER TABLE bytecodes RENAME COLUMN type TO bytecode_type;
        ALTER TABLE parts RENAME COLUMN type TO part_type;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
        ALTER TABLE bytecodes RENAME COLUMN bytecode_type TO type;
        ALTER TABLE parts RENAME COLUMN part_type TO type;
        "#;
        crate::from_sql(manager, sql).await
    }
}
