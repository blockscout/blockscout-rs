use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "eigenda_v2_commitments" (
                -- keccak256 of "commitment_data"
                "commitment_hash" bytea PRIMARY KEY,
                "commitment_data" bytea NOT NULL,
                "blob_data" bytea,
                "blob_data_s3_object_key" varchar,

                CONSTRAINT "blob_data_integrity"
                    CHECK ("blob_data" IS NOT NULL AND "blob_data_s3_object_key" IS NULL
                        OR "blob_data" IS NULL AND "blob_data_s3_object_key" IS NOT NULL)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "eigenda_v2_commitments";
        "#;
        crate::from_sql(manager, sql).await
    }
}
