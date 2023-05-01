use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "contract_addresses" (
              "contract_address" bytea PRIMARY KEY NOT NULL,
              "verification_method" verification_method NOT NULL,
              "status" status NOT NULL DEFAULT 'waiting',
              "log" varchar,
              "creation_input" bytea
            );
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"DROP TABLE contract_addresses;"#;

        crate::from_sql(manager, sql).await
    }
}
