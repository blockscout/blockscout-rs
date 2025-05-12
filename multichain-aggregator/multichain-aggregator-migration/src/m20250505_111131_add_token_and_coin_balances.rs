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
                address_hash bytea NOT NULL,
                token_address_hash bytea NOT NULL,
                value NUMERIC(78, 0) NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),

                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),

                PRIMARY KEY (address_hash, chain_id, token_address_hash)
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
