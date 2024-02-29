use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE EXTENSION pgcrypto;
            
            CREATE TYPE "public_tag_type" AS ENUM (
                'name',
                'generic',
                'information'
                );

            CREATE TABLE "public_tags"
            (
                "id"                uuid PRIMARY KEY         DEFAULT gen_random_uuid(),
                
                "created_at"        timestamp       NOT NULL DEFAULT (now()),
                "created_by"        varchar         NOT NULL DEFAULT (current_user),
                "modified_at"       timestamp       NOT NULL DEFAULT (now()),
                "modified_by"       varchar         NOT NULL DEFAULT (current_user),
                
                "source"            varchar         NOT NULL,
                "slug"              varchar(255)    NOT NULL,
                "name"              varchar(255)    NOT NULL,
                "tag_type"          public_tag_type NOT NULL,
                "ordinal"           integer         NOT NULL DEFAULT (0),
                "reputation_impact" integer,
                "meta"              jsonb           NOT NULL DEFAULT ('{}'),
                CONSTRAINT "public_tags_pseudo_pkey" UNIQUE ("created_by", "slug", "tag_type")
            );
            
            CREATE TABLE "address_public_tags"
            (
                "id"             uuid PRIMARY KEY   DEFAULT gen_random_uuid(),
            
                "created_at"     timestamp NOT NULL DEFAULT (now()),
                "created_by"     varchar   NOT NULL DEFAULT (current_user),
                "modified_at"    timestamp NOT NULL DEFAULT (now()),
                "modified_by"    varchar   NOT NULL DEFAULT (current_user),
                
                "source"         varchar   NOT NULL,
                "address"        bytea     NOT NULL,
                "chain_id"       integer,
                "public_tag_id"  uuid      NOT NULL REFERENCES "public_tags" ("id"),
                "overrided_meta" jsonb     NOT NULL DEFAULT ('{}'),
            
                CONSTRAINT "address_public_tags_pseudo_pkey" UNIQUE ("created_by", "address", "chain_id", "public_tag_id")
            );
            
            CREATE UNIQUE INDEX "address_public_tags_null_chain_pseudo_pkey" ON "address_public_tags" ("created_by", "address", "public_tag_id") WHERE "chain_id" IS NULL;            
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "address_public_tags";
            DROP TABLE "public_tags";

            DROP TYPE "public_tag_type";
            DROP EXTENSION "pgcrypto";
        "#;
        crate::from_sql(manager, sql).await
    }
}
