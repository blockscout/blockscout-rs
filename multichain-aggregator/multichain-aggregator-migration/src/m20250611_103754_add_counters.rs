use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE counters_global_imported (
                id bigserial NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),
                daily_transactions_number bigint DEFAULT NULL,
                total_transactions_number bigint DEFAULT NULL,
                total_addresses_number bigint DEFAULT NULL,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );

            CREATE INDEX interop_messages_sender_address_hash_index ON interop_messages (sender_address_hash);
            CREATE INDEX interop_messages_target_address_hash_index ON interop_messages (target_address_hash);
            CREATE INDEX interop_messages_transfers_from_address_hash_index ON interop_messages_transfers (from_address_hash);
            CREATE INDEX interop_messages_transfers_to_address_hash_index ON interop_messages_transfers (to_address_hash);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let sql = r#"
            DROP TABLE IF EXISTS counters_global_imported;
            DROP TABLE IF EXISTS counters_token_imported;

            ALTER TABLE addresses DROP COLUMN counter_interop_messages;
            ALTER TABLE addresses DROP COLUMN counter_interop_transfers;

            DROP INDEX IF EXISTS interop_messages_sender_address_hash_index;
            DROP INDEX IF EXISTS interop_messages_target_address_hash_index;
            DROP INDEX IF EXISTS interop_messages_transfers_from_address_hash_index;
            DROP INDEX IF EXISTS interop_messages_transfers_to_address_hash_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
