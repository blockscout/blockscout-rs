use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // TODO: add indexes
        let sql = r#"
            CREATE TABLE interop_messages (
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
                transfer_token_address_hash bytea,
                transfer_from_address_hash bytea,
                transfer_to_address_hash bytea,
                transfer_amount NUMERIC(78, 0),
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (init_chain_id, nonce)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE interop_messages;
        "#;
        crate::from_sql(manager, sql).await
    }
}
