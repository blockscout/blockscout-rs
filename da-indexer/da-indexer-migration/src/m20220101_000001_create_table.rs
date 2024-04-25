use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "celestia_blocks" (
                "height" bigint PRIMARY KEY,
                "hash" bytea NOT NULL,
                "blobs_count" integer NOT NULL,
                "timestamp" bigint NOT NULL
            );
            
            CREATE TABLE "celestia_blobs" (
                "id" bytea PRIMARY KEY,
                "height" bigint NOT NULL references "celestia_blocks"("height"),
                "namespace" bytea NOT NULL,
                "commitment" bytea NOT NULL,
                "data" bytea NOT NULL
            );

            COMMENT ON TABLE "celestia_blocks" IS 'Table contains blocks metadata';

            COMMENT ON TABLE "celestia_blobs" IS 'Table contains celestia blobs';
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "celestia_blobs";
            DROP TABLE "celestia_blocks";
        "#;

        crate::from_sql(manager, sql).await
    }
}
