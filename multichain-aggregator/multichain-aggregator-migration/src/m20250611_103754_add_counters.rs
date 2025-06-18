use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE counters_global_imported (
                id bigserial NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),
                timestamp timestamp NOT NULL DEFAULT (now()),
                daily_transactions_number bigint DEFAULT NULL,
                total_transactions_number bigint DEFAULT NULL,
                total_addresses_number bigint DEFAULT NULL,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );

            CREATE TABLE counters_token_imported (
                id bigserial NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),
                timestamp timestamp NOT NULL DEFAULT (now()),
                daily_transactions_number bigint NOT NULL DEFAULT 0,
                total_transactions_number bigint NOT NULL DEFAULT 0,
                total_addresses_number bigint NOT NULL DEFAULT 0,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );

            CREATE TABLE counters_interop (
                id bigserial NOT NULL,
                address_hash bytea NOT NULL,
                total_messages bigint NOT NULL DEFAULT 0,
                total_transfers bigint NOT NULL DEFAULT 0,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let sql = r#"
            DROP TABLE counters_global_imported;
            DROP TABLE counters_token_imported;
            DROP TABLE counters_interop;
        "#;
        crate::from_sql(manager, sql).await
    }
}
