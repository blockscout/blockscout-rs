use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE TABLE counters_global_imported (
                id bigserial NOT NULL,
                chain_id bigint NOT NULL REFERENCES chains (id),
                date date NOT NULL,
                daily_transactions_number bigint DEFAULT NULL,
                total_transactions_number bigint DEFAULT NULL,
                total_addresses_number bigint DEFAULT NULL,
                created_at timestamp NOT NULL DEFAULT (now()),
                updated_at timestamp NOT NULL DEFAULT (now()),
                PRIMARY KEY (id),
                UNIQUE (chain_id, date)
            );

            CREATE INDEX counters_global_imported_chain_id_date_index ON counters_global_imported (chain_id, date);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts
        let sql = r#"
            DROP TABLE IF EXISTS counters_global_imported;

            DROP INDEX IF EXISTS counters_global_imported_chain_id_date_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
