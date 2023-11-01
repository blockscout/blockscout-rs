use sea_orm_migration::{
    prelude::*,
    sea_orm::{Statement, TransactionTrait},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let create_extension_pgcrypto = r#"
            -- Needed for gen_random_uuid()
            CREATE EXTENSION IF NOT EXISTS "pgcrypto";
        "#;
        let create_function_job_queue_set_modified_at = r#"
            CREATE OR REPLACE FUNCTION _job_queue_set_modified_at()
                RETURNS TRIGGER AS
            $$
            BEGIN
                NEW.modified_at = now();
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;
        let create_enum_job_status = r#"
            CREATE TYPE "_job_status" AS ENUM (
                'waiting',
                'in_process',
                'success',
                'error'
            );
        "#;
        let create_table_job_queue = r#"
            CREATE TABLE "_job_queue" (
                "id" uuid PRIMARY KEY DEFAULT gen_random_uuid(),

                "created_at" timestamp NOT NULL DEFAULT (now()),
                "modified_at" timestamp NOT NULL DEFAULT (now()),

                "status" _job_status NOT NULL DEFAULT 'waiting',
                "log" varchar
            );
        "#;
        let create_index_job_queue_status =
            "CREATE INDEX _job_queue_status_index ON _job_queue (status);";
        let create_trigger_job_queue_set_modified_at = r#"
            CREATE TRIGGER "set_modified_at"
            BEFORE UPDATE ON "_job_queue"
                FOR EACH ROW
            EXECUTE FUNCTION _job_queue_set_modified_at();
        "#;
        let create_function_insert_job = r#"
            CREATE OR REPLACE FUNCTION _insert_job()
            RETURNS TRIGGER AS $$
            BEGIN
              -- Insert a new row into the jobs table and get the ID
              INSERT INTO _job_queue DEFAULT VALUES
              RETURNING id INTO NEW._job_id;

              -- Update the jobs_id in the contract_addresses table
              RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;

        from_statements(
            manager,
            &[
                create_extension_pgcrypto,
                create_function_job_queue_set_modified_at,
                create_enum_job_status,
                create_table_job_queue,
                create_index_job_queue_status,
                create_trigger_job_queue_set_modified_at,
                create_function_insert_job,
            ],
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let drop_function_insert_job = "DROP FUNCTION _insert_job;";
        let drop_trigger_job_queue_set_modified_at = "DROP TRIGGER set_modified_at ON _job_queue;";
        let drop_index_job_queue_status = "DROP INDEX _job_queue_status_index;";
        let drop_table_job_queue = "DROP TABLE _job_queue;";
        let drop_enum_job_status = "DROP TYPE _job_status;";
        let drop_function_job_queue_set_modified_at = "DROP FUNCTION _job_queue_set_modified_at;";
        let drop_extension_pgcrypto = "DROP EXTENSION pgcrypto;";

        from_statements(
            manager,
            &[
                drop_function_insert_job,
                drop_trigger_job_queue_set_modified_at,
                drop_index_job_queue_status,
                drop_table_job_queue,
                drop_enum_job_status,
                drop_function_job_queue_set_modified_at,
                drop_extension_pgcrypto,
            ],
        )
        .await
    }
}

pub fn create_job_queue_connection_statements(relation: &str) -> Vec<String> {
    let create_trigger_insert_job = format!(
        r#"
            CREATE TRIGGER insert_job
            BEFORE INSERT ON {relation}
                FOR EACH ROW
            EXECUTE FUNCTION _insert_job();
        "#
    );
    let create_index_on_job_id =
        format!("CREATE INDEX _{relation}_job_id_index ON {relation} (_job_id);");

    vec![create_trigger_insert_job, create_index_on_job_id]
}

pub fn drop_job_queue_connection_statements(relation: &str) -> Vec<String> {
    let drop_trigger_insert_job = format!("DROP TRIGGER insert_job ON {relation};");
    let drop_index_on_job_id = format!("DROP INDEX _{relation}_job_id_index;");

    vec![drop_index_on_job_id, drop_trigger_insert_job]
}

async fn from_statements(manager: &SchemaManager<'_>, statements: &[&str]) -> Result<(), DbErr> {
    let txn = manager.get_connection().begin().await?;
    for statement in statements {
        txn.execute(Statement::from_string(
            manager.get_database_backend(),
            statement.to_string(),
        ))
        .await
        .map_err(|err| DbErr::Migration(format!("{err}\nQuery: {statement}")))?;
    }
    txn.commit().await
}
