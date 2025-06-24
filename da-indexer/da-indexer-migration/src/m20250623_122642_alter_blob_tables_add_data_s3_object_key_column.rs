use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "celestia_blobs"
                ALTER COLUMN "data" DROP NOT NULL,
                ADD COLUMN "data_s3_object_key" varchar;

            ALTER TABLE "eigenda_blobs"
                ALTER COLUMN "data" DROP NOT NULL,
                ADD COLUMN "data_s3_object_key" varchar;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "celestia_blobs"
                DROP COLUMN "data_s3_object_key",
                ALTER COLUMN "data" SET NOT NULL;

            ALTER TABLE "eigenda_blobs"
                DROP COLUMN "data_s3_object_key",
                ALTER COLUMN "data" SET NOT NULL;
        "#;

        crate::from_sql(manager, sql).await
    }
}
