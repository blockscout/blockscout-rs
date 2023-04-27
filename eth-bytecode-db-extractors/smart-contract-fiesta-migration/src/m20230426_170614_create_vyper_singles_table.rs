use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "vyper_singles" (
              "contract_address" bytea PRIMARY KEY NOT NULL,
              "contract_name" varchar NOT NULL,
              "compiler_version" varchar NOT NULL,
              "optimizations" bool NOT NULL,
              "optimization_runs" bigint NOT NULL,
              "source_code" varchar NOT NULL
            );

            ALTER TABLE "vyper_singles" ADD FOREIGN KEY ("contract_address") REFERENCES "contract_addresses" ("contract_address");
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"DROP TABLE vyper_singles;"#;

        crate::from_sql(manager, sql).await
    }
}
