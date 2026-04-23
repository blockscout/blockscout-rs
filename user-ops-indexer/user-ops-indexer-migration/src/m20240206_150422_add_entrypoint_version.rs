use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TYPE "entry_point_version" AS ENUM (
              'v0.6',
              'v0.7'
            );

            ALTER TABLE "user_operations" ADD COLUMN "entry_point_version" entry_point_version DEFAULT 'v0.6' NOT NULL;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "user_operations" DROP COLUMN "entry_point_version";

            DROP TYPE "entry_point_version";
        "#;
        crate::from_sql(manager, sql).await
    }
}
