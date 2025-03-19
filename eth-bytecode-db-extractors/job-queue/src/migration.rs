use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = include_str!("../sql/up.sql");
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = include_str!("../sql/down.sql");
        manager.get_connection().execute_unprepared(sql).await?;
        Ok(())
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
