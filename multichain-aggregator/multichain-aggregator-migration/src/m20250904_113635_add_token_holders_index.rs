use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX IF NOT EXISTS address_token_balances_token_address_hash_chain_id_value ON address_token_balances (
                token_address_hash,
                chain_id,
                value
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS address_token_balances_token_address_hash_chain_id_value;
        "#;
        crate::from_sql(manager, sql).await
    }
}
