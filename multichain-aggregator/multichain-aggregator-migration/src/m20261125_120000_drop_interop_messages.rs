// SPDX-License-Identifier: LicenseRef-Blockscout

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE IF EXISTS interop_messages_transfers;
            DROP TABLE IF EXISTS interop_messages;
        "#;
        crate::from_sql(manager, sql).await
    }

    // Recreates the empty tables as they were before this migration
    // (without the chain foreign keys dropped in m20250602_105925).
    // The data itself is not restored.
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE interop_messages (
                id bigserial NOT NULL,
                sender_address_hash bytea,
                target_address_hash bytea,
                nonce bigint NOT NULL,
                init_chain_id bigint NOT NULL,
                init_transaction_hash bytea,
                timestamp timestamp,
                relay_chain_id bigint NOT NULL,
                relay_transaction_hash bytea,
                payload bytea,
                failed boolean,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );
            CREATE UNIQUE INDEX interop_messages_init_chain_id_nonce_unique_index ON interop_messages (nonce, init_chain_id);
            CREATE INDEX interop_messages_timestamp_init_transaction_hash_index ON interop_messages (timestamp, init_transaction_hash);

            CREATE TABLE interop_messages_transfers (
                interop_message_id bigint NOT NULL REFERENCES interop_messages (id),
                token_address_hash bytea,
                from_address_hash bytea NOT NULL,
                to_address_hash bytea NOT NULL,
                amount NUMERIC(78, 0) NOT NULL,
                PRIMARY KEY (interop_message_id)
            );
        "#;
        crate::from_sql(manager, sql).await
    }
}
