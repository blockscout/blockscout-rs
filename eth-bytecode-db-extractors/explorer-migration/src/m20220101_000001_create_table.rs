use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let create_contract_addresses_table = r#"
            CREATE TABLE "contracts" (
                "address" bytea NOT NULL PRIMARY KEY,
                "inserted_at" timestamp NOT NULL DEFAULT now(),
                "updated_at" timestamp NOT NULL DEFAULT now(),
                "is_verified" bool,
                "data" jsonb,
                "_job_id" bigint NOT NULL REFERENCES "_job_queue" ("id")
            );
        "#;

        let create_job_queue_connection_statements =
            job_queue::migration::create_job_queue_connection_statements("contracts");

        let mut statements = vec![create_contract_addresses_table];
        statements.extend(
            create_job_queue_connection_statements
                .iter()
                .map(|v| v.as_str()),
        );

        crate::from_statements(manager, &statements).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let drop_job_queue_connection_statements =
            job_queue::migration::drop_job_queue_connection_statements("contracts");
        let drop_table_contract_addresses = "DROP TABLE contracts;";

        let mut statements = drop_job_queue_connection_statements
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        statements.extend([drop_table_contract_addresses]);

        crate::from_statements(manager, &statements).await
    }
}
