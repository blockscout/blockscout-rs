use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX IF NOT EXISTS tokens_list_index ON tokens (
                circulating_market_cap DESC NULLS LAST,
                fiat_value DESC NULLS LAST,
                holders_count DESC NULLS LAST,
                name ASC NULLS LAST,
                address_hash ASC NULLS LAST,
                chain_id ASC NULLS LAST
            ) WHERE token_type <> 'ERC-7802'::token_type;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS tokens_list_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
