use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let create_contract_addresses_table = r#"
            CREATE TABLE "contract_addresses" (
                "contract_address" bytea NOT NULL,

                "fetched_coin_balance_block_number" bigint NOT NULL,
                "inserted_at" timestamp NOT NULL,
                "updated_at" timestamp NOT NULL,

                "_job_id" uuid NOT NULL REFERENCES "_job_queue" ("id"),

                PRIMARY KEY ("contract_address")
            );
        "#;

        let create_job_queue_connection_statements =
            job_queue::migration::create_job_queue_connection_statements("contract_addresses");

        let mut statements = vec![
            create_contract_addresses_table,
        ];
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
