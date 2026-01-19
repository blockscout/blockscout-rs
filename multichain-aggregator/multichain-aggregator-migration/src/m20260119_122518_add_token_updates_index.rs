use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX IF NOT EXISTS tokens_updates_index ON tokens (
                updated_at ASC,
                address_hash ASC,
                chain_id ASC
            );
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX IF EXISTS tokens_updates_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
