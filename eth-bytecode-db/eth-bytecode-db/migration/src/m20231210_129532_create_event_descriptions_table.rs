use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
             CREATE TABLE "events" (
              "id" BIGSERIAL PRIMARY KEY,
              "created_at" timestamp NOT NULL DEFAULT (now()),
              "updated_at" timestamp NOT NULL DEFAULT (now()),
              "selector" bytea NOT NULL,
              "name" varchar NOT NULL,
              "inputs" jsonb NOT NULL
            );

            CREATE UNIQUE INDEX "unique_events_name_and_inputs_index" ON "events" ("name", md5("inputs"::text));

            CREATE INDEX "events_selector_index" ON "events" ("selector");
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "events";
        "#;
        crate::from_sql(manager, sql).await
    }
}
