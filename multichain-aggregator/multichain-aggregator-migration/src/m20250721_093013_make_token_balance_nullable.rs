use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE address_token_balances ALTER COLUMN value DROP NOT NULL;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            UPDATE address_token_balances SET value = 0 WHERE value IS NULL;
            ALTER TABLE address_token_balances ALTER COLUMN value SET NOT NULL;
        "#;
        crate::from_sql(manager, sql).await
    }
}
