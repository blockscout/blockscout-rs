use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE block_ranges ALTER COLUMN min_block_number TYPE bigint;
            ALTER TABLE block_ranges ALTER COLUMN max_block_number TYPE bigint;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE block_ranges ALTER COLUMN min_block_number TYPE integer;
            ALTER TABLE block_ranges ALTER COLUMN max_block_number TYPE integer;
        "#;
        crate::from_sql(manager, sql).await
    }
}
