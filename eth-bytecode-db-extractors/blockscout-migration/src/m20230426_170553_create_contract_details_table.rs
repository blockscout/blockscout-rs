use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "contract_details" (
              "created_at" timestamp NOT NULL DEFAULT (now()),
              "modified_at" timestamp NOT NULL DEFAULT (now()),

              "contract_address" bytea NOT NULL,
              "chain_id" numeric NOT NULL,

              "sources" jsonb NOT NULL,
              "settings" jsonb,

              "verified_via_sourcify" bool NOT NULL DEFAULT false,
              "optimization_enabled" bool,
              "optimization_runs" bigint,
              "evm_version" varchar,
              "libraries" jsonb,

              "creation_code" bytea,
              "runtime_code" bytea NOT NULL,
              "transaction_hash" bytea,
              "block_number" numeric NOT NULL,
              "transaction_index" numeric,
              "deployer" bytea,

              PRIMARY KEY ("contract_address", "chain_id")
            );

            ALTER TABLE "contract_details" ADD FOREIGN KEY ("contract_address", "chain_id") REFERENCES "contract_addresses" ("contract_address", "chain_id");

            CREATE TRIGGER trigger_set_modified_at
            BEFORE INSERT ON contract_details
                FOR EACH ROW
            EXECUTE FUNCTION set_modified_at();
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"DROP TABLE contract_details;"#;

        crate::from_sql(manager, sql).await
    }
}
