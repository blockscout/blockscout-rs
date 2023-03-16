use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "verified_contracts"
            ADD COLUMN "chain_id" bigint,
            ADD COLUMN "contract_address" bytea;
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE "verified_contracts"
            DROP COLUMN "chain_id",
            DROP COLUMN "contract_address";
        "#;
        crate::from_sql(manager, sql).await
    }
}
