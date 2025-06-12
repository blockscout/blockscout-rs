use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE address_coin_balances (
                address_hash bytea NOT NULL,
                value NUMERIC(78, 0) NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),

                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),

                PRIMARY KEY (address_hash, chain_id)
            );

            CREATE TABLE address_token_balances (
                id bigserial NOT NULL,
                address_hash bytea NOT NULL,
                token_address_hash bytea NOT NULL,
                token_id numeric(78),
                value NUMERIC(78, 0) NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),

                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),

                PRIMARY KEY (id)
            );
            CREATE UNIQUE INDEX address_token_balances_address_hash_chain_id_token_address_hash_token_id_unique_index ON address_token_balances (
                address_hash,
                chain_id,
                token_address_hash,
                COALESCE(token_id, '-1'::integer::numeric)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE address_token_balances;
            DROP TABLE address_coin_balances;
        "#;
        crate::from_sql(manager, sql).await
    }
}
