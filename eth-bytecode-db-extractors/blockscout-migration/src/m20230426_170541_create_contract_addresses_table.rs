use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE "contract_addresses" (
                "contract_address" bytea NOT NULL,
                "chain_id" numeric NOT NULL,

                "created_at" timestamp NOT NULL DEFAULT (now()),
                "modified_at" timestamp NOT NULL DEFAULT (now()),

                "verified_at" timestamptz NOT NULL,
                "language" language NOT NULL,
                "compiler_version" varchar NOT NULL,

                "status" status NOT NULL DEFAULT 'waiting',
                "log" varchar,

                PRIMARY KEY ("contract_address", "chain_id")
            );

            CREATE TRIGGER trigger_set_modified_at
            BEFORE INSERT ON contract_addresses
                FOR EACH ROW
            EXECUTE FUNCTION set_modified_at();
        "#;

        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TRIGGER trigger_set_modified_at ON contract_addresses;
            DROP TABLE contract_addresses;
        "#;

        crate::from_sql(manager, sql).await
    }
}
