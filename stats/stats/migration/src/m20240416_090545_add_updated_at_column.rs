use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE charts
                ADD COLUMN last_updated_at timestamp;

            UPDATE charts SET last_updated_at = (
                SELECT max(date) FROM chart_data
                WHERE charts.id = chart_id
                GROUP BY chart_id
            );"#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE charts
                DROP COLUMN last_updated_at;"#;
        crate::from_sql(manager, sql).await
    }
}
