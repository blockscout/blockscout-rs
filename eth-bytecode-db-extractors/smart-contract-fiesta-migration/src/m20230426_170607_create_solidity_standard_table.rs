use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "solidity_standards" (
              "contract_address" bytea PRIMARY KEY NOT NULL,
              "contract_name" varchar NOT NULL,
              "compiler_version" varchar NOT NULL,
              "standard_json" jsonb NOT NULL
            );

            ALTER TABLE "solidity_standards" ADD FOREIGN KEY ("contract_address") REFERENCES "contract_addresses" ("contract_address");
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"DROP TABLE solidity_standards;"#;

        crate::from_sql(manager, sql).await
    }
}
