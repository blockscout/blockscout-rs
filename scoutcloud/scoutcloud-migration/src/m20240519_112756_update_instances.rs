use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TABLE "instances" DROP CONSTRAINT "instances_slug_key";
            ALTER TABLE "instances" ADD COLUMN "deleted" BOOL NOT NULL DEFAULT FALSE;
            "#,
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TABLE "instances" ADD CONSTRAINT "instances_slug_key" UNIQUE ("slug");
            ALTER TABLE instances DROP COLUMN deleted;
            "#,
        )
        .await
    }
}
