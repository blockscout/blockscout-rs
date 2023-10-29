use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let create_contract_addresses_table = r#"
            CREATE TABLE "contract_addresses" (
                "contract_address" bytea NOT NULL,
                "chain_id" numeric NOT NULL,

                "created_at" timestamp NOT NULL DEFAULT (now()),
                "modified_at" timestamp NOT NULL DEFAULT (now()),

                "verified_at" timestamptz NOT NULL,
                "language" language NOT NULL,
                "compiler_version" varchar NOT NULL,

                "_job_id" uuid NOT NULL REFERENCES "_job_queue" ("id"),

                PRIMARY KEY ("contract_address", "chain_id")
            );
        "#;
        let create_trigger_set_modified_at = r#"
            CREATE TRIGGER trigger_set_modified_at
            BEFORE UPDATE ON contract_addresses
                FOR EACH ROW
            EXECUTE FUNCTION set_modified_at();
        "#;
        let create_trigger_insert_into_job_queue =
            job_queue::migration::create_trigger_insert_job_statement("contract_addresses");

        crate::from_statements(
            manager,
            &[
                create_contract_addresses_table,
                create_trigger_set_modified_at,
                &create_trigger_insert_into_job_queue,
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let drop_trigger_insert_into_job_queue =
            job_queue::migration::drop_trigger_insert_job_statement("contract_addresses");
        let drop_trigger_set_modified_at =
            "DROP TRIGGER trigger_set_modified_at ON contract_addresses;";
        let drop_table_contract_addresses = "DROP TABLE contract_addresses;";

        crate::from_statements(
            manager,
            &[
                &drop_trigger_insert_into_job_queue,
                drop_trigger_set_modified_at,
                drop_table_contract_addresses,
            ],
        )
        .await
    }
}
