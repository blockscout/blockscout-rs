use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TYPE token_type ADD VALUE IF NOT EXISTS 'ERC-7802';

            CREATE TABLE tokens (
                address_hash bytea NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),

                name text,
                symbol text,
                decimals smallint,
                token_type token_type NOT NULL,

                icon_url text,

                fiat_value numeric,
                circulating_market_cap numeric,

                total_supply numeric(78, 0),
                holders_count bigint,
                transfers_count bigint,

                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),

                PRIMARY KEY (address_hash, chain_id)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE tokens;
        "#;
        crate::from_sql(manager, sql).await
    }
}
