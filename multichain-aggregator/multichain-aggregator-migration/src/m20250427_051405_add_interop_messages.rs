use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TODO: add indexes
        let sql = r#"
            CREATE TABLE interop_messages (
                id bigserial NOT NULL,
                sender_address_hash bytea,
                target_address_hash bytea,
                nonce bigint NOT NULL,
                init_chain_id bigint NOT NULL REFERENCES chains (id),
                init_transaction_hash bytea,
                timestamp timestamp,
                relay_chain_id bigint NOT NULL REFERENCES chains (id),
                relay_transaction_hash bytea,
                payload bytea,
                failed boolean,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id)
            );
            CREATE UNIQUE INDEX interop_messages_init_chain_id_nonce_unique_index ON interop_messages (init_chain_id, nonce);

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

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE interop_messages_transfers;
            DROP INDEX interop_messages_init_chain_id_nonce_unique_index;
            DROP TABLE interop_messages;
        "#;
        crate::from_sql(manager, sql).await
    }
}
