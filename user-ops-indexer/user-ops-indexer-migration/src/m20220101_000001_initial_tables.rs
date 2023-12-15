use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("blocks").await? {
            return Err(DbErr::Migration(
                "Table blocks does not exist in the database".to_string(),
            ));
        }
        if !manager.has_table("logs").await? {
            return Err(DbErr::Migration(
                "Table logs does not exist in the database".to_string(),
            ));
        }

        let sql = r#"
            CREATE TYPE "sponsor_type" AS ENUM (
              'wallet_deposit',
              'wallet_balance',
              'paymaster_sponsor',
              'paymaster_hybrid'
            );

            CREATE TABLE "user_operations" (
              "hash" bytea PRIMARY KEY,

              -- EIP4337 struct raw fields
              "sender" bytea NOT NULL, -- AA wallet address
              "nonce" bytea NOT NULL, -- uint192 key ++ uint64 sequence
              "init_code" bytea DEFAULT NULL, -- (optional) address factory ++ bytes initcode
              "call_data" bytea NOT NULL,
              "call_gas_limit" NUMERIC(100) NOT NULL,
              "verification_gas_limit" NUMERIC(100) NOT NULL,
              "pre_verification_gas" NUMERIC(100) NOT NULL,
              "max_fee_per_gas" NUMERIC(100) NOT NULL,
              "max_priority_fee_per_gas" NUMERIC(100) NOT NULL,
              "paymaster_and_data" bytea DEFAULT NULL, -- (optional) address paymaster ++ bytes data
              "signature" bytea NOT NULL,

              "aggregator" bytea DEFAULT NULL,
              "aggregator_signature" bytea DEFAULT NULL,

              -- context fields
              "entry_point" bytea NOT NULL,
              "transaction_hash" bytea NOT NULL,
              "block_number" int NOT NULL,
              "block_hash" bytea NOT NULL,
              "bundle_index" int NOT NULL,
              "index" int NOT NULL,
              "user_logs_start_index" int NOT NULL,
              "user_logs_count" int NOT NULL,

              -- derived fields
              "bundler" bytea NOT NULL,
              "factory" bytea DEFAULT NULL,
              "paymaster" bytea DEFAULT NULL,
              "status" bool NOT NULL,
              "revert_reason" bytea DEFAULT NULL,
              "gas" NUMERIC(100) NOT NULL,
              "gas_price" NUMERIC(100) NOT NULL,
              "gas_used" NUMERIC(100) NOT NULL,

              "sponsor_type" sponsor_type NOT NULL,

              "inserted_at" timestamp NOT NULL DEFAULT (now()),
              "updated_at" timestamp NOT NULL DEFAULT (now())
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE "user_operations";

            DROP TYPE "sponsor_type";
        "#;

        crate::from_sql(manager, sql).await
    }
}
