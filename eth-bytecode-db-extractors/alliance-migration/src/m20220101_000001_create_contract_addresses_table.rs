use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let create_contract_addresses_table = r#"
            CREATE TABLE "contract_addresses" (
                "id" bigserial PRIMARY KEY,
                "chain_id" numeric NOT NULL,
                "address" bytea NOT NULL,

                "compiler" varchar NOT NULL,
                "version" varchar NOT NULL,
                "language" varchar NOT NULL,

                "sources" jsonb NOT NULL,
                "compiler_settings" jsonb NOT NULL,

                "_job_id" uuid NOT NULL REFERENCES "_job_queue" ("id")
            );
        "#;

        let create_job_queue_connection_statements =
            job_queue::migration::create_job_queue_connection_statements("contract_addresses");

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
            job_queue::migration::drop_job_queue_connection_statements("contract_addresses");
        let drop_table_contract_addresses = "DROP TABLE contract_addresses;";

        let mut statements = drop_job_queue_connection_statements
            .iter()
            .map(|v| v.as_str())
            .collect::<Vec<_>>();
        statements.extend([drop_table_contract_addresses]);

        crate::from_statements(manager, &statements).await
    }
}
