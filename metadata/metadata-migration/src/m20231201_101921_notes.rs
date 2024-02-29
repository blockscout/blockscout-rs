use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TYPE "note_severity_level" AS ENUM (
                'info',
                'warn'
                );
            
            CREATE TABLE "notes"
            (
                "id"          uuid PRIMARY KEY             DEFAULT gen_random_uuid(),
            
                "created_at"  timestamp           NOT NULL DEFAULT (now()),
                "created_by"  varchar             NOT NULL DEFAULT (current_user),
                "modified_at" timestamp           NOT NULL DEFAULT (now()),
                "modified_by" varchar             NOT NULL DEFAULT (current_user),
            
                "source"      varchar             NOT NULL,
                "text"        varchar             NOT NULL,
                "severity"    note_severity_level NOT NULL DEFAULT ('info'),
                "ordinal"     integer             NOT NULL DEFAULT (0),
                "meta"        jsonb               NOT NULL DEFAULT ('{}')
            );
            
            CREATE UNIQUE INDEX "notes_pseudo_pkey" on "notes" ("created_by", SHA256("text"::bytea));
            
            CREATE TABLE "address_notes"
            (
                "id"          uuid PRIMARY KEY   DEFAULT gen_random_uuid(),
            
                "created_at"  timestamp NOT NULL DEFAULT (now()),
                "created_by"  varchar   NOT NULL DEFAULT (current_user),
                "modified_at" timestamp NOT NULL DEFAULT (now()),
                "modified_by" varchar   NOT NULL DEFAULT (current_user),
            
                "source"      varchar   NOT NULL,
                "address"     bytea     NOT NULL,
                "chain_id"    integer,
                "note_id"     uuid      NOT NULL REFERENCES "notes" ("id"),
                CONSTRAINT "address_notes_pseudo_pkey" UNIQUE ("created_by", "address", "chain_id", "note_id")
            );
            
            CREATE UNIQUE INDEX "address_notes_null_chain_pseudo_pkey" ON "address_notes" ("created_by", "address", "note_id") WHERE "chain_id" IS NULL;
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "address_notes";
            DROP TABLE "notes";

            DROP TYPE "note_severity_level";
        "#;
        crate::from_sql(_manager, sql).await
    }
}
