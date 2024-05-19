use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
        ALTER TABLE users
        ADD COLUMN max_instances INTEGER NOT NULL DEFAULT 20;
        "#,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
        ALTER TABLE users
        DROP COLUMN max_instances;
        "#,
        )
        .await
    }
}
