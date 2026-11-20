// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Unlike "events", functions are unique per canonical signature
        // (e.g. `transfer(address,uint256)`) and not per the names of their inputs.
        // Otherwise popular selectors would accumulate a row for every
        // spelling of the argument names seen in verified contracts.
        let sql = r#"
             CREATE TABLE "functions" (
              "id" BIGSERIAL PRIMARY KEY,
              "created_at" timestamp NOT NULL DEFAULT (now()),
              "updated_at" timestamp NOT NULL DEFAULT (now()),
              "selector" bytea NOT NULL,
              "name" varchar NOT NULL,
              "signature" varchar NOT NULL,
              "inputs" jsonb NOT NULL
            );

            CREATE UNIQUE INDEX "unique_functions_signature_index" ON "functions" (md5("signature"));

            CREATE INDEX "functions_selector_index" ON "functions" ("selector");
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "functions";
        "#;
        crate::from_sql(manager, sql).await
    }
}
