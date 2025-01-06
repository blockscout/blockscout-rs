use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "eigenda_batches" (
                "batch_id" bigint PRIMARY KEY,
                "batch_header_hash" bytea NOT NULL,
                "blobs_count" integer NOT NULL,
                "l1_tx_hash" bytea NOT NULL,
                "l1_block" bigint NOT NULL
            );
            
            CREATE TABLE "eigenda_blobs" (
                "id" bytea PRIMARY KEY,
                "batch_header_hash" bytea NOT NULL,
                "blob_index" integer NOT NULL,
                "data" bytea NOT NULL
            );

            COMMENT ON TABLE "eigenda_batches" IS 'Table contains eigenda batches metadata';

            COMMENT ON TABLE "eigenda_blobs" IS 'Table contains eigenda blobs';
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "eigenda_batches";
            DROP TABLE "eigenda_blobs";
        "#;

        crate::from_sql(manager, sql).await
    }
}
