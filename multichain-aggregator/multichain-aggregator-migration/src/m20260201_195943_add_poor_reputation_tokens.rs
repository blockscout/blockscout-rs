use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE poor_reputation_tokens (
                address_hash bytea NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),
                created_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (address_hash, chain_id)
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TABLE IF EXISTS poor_reputation_tokens;
        "#;
        crate::from_sql(manager, sql).await
    }
}
