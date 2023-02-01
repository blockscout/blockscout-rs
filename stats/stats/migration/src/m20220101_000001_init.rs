use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
CREATE TYPE "chart_type" AS ENUM (
  'COUNTER',
  'LINE'
);

CREATE TABLE "charts" (
  "id" INT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
  "name" varchar(256) UNIQUE NOT NULL,
  "chart_type" chart_type NOT NULL,
  "created_at" timestamp NOT NULL DEFAULT (now())
);

CREATE TABLE "chart_data" (
  "id" INT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
  "chart_id" int NOT NULL,
  "date" date NOT NULL,
  "value" varchar(64) NOT NULL,
  "created_at" timestamp NOT NULL DEFAULT (now())
);

CREATE UNIQUE INDEX ON "chart_data" ("chart_id", "date");

COMMENT ON TABLE "charts" IS 'Table contains chart description and sync info';

COMMENT ON TABLE "chart_data" IS 'Table contains chart data points';

ALTER TABLE "chart_data" ADD FOREIGN KEY ("chart_id") REFERENCES "charts" ("id");
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
DROP TABLE "chart_data";

DROP TABLE "charts";

DROP TYPE "chart_type";
        "#;
        crate::from_sql(manager, sql).await
    }
}
