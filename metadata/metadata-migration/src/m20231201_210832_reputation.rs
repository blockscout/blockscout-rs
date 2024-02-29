use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "address_reputation"
            (
                "id"          uuid PRIMARY KEY   DEFAULT gen_random_uuid(),
            
                "created_at"  timestamp NOT NULL DEFAULT (now()),
                "created_by"  varchar   NOT NULL DEFAULT (current_user),
                "modified_at" timestamp NOT NULL DEFAULT (now()),
                "modified_by" varchar   NOT NULL DEFAULT (current_user),
            
                "source"      varchar   NOT NULL,
                "address"     bytea     NOT NULL,
                "chain_id"    integer,
                "reputation"  integer   NOT NULL DEFAULT (0),
                CONSTRAINT "address_reputation_pseudo_pkey" UNIQUE ("created_by", "address", "chain_id")
            );
            
            CREATE UNIQUE INDEX "address_reputation_null_chain_pseudo_pkey" ON "address_notes" ("created_by", "address") WHERE "chain_id" IS NULL;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "address_reputation";
        "#;

        crate::from_sql(manager, sql).await
    }
}
