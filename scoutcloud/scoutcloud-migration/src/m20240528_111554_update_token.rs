use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TABLE auth_tokens ADD COLUMN name VARCHAR NOT NULL DEFAULT 'auth token';
            ALTER TABLE auth_tokens ALTER COLUMN name DROP DEFAULT;
            "#,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TABLE "auth_tokens" DROP COLUMN "name";
            "#,
        )
        .await
    }
}
