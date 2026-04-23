use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TYPE "verification_method" AS ENUM (
              'solidity_single',
              'solidity_multiple',
              'solidity_standard',
              'vyper_single'
            );
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"DROP TYPE "verification_method";"#;

        crate::from_sql(manager, sql).await
    }
}
