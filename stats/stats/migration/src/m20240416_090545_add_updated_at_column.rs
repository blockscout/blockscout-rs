use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // add & populate column
        // fix timezone of timestamps (everywhere in the code UTC is used)
        let sql = r#"
            ALTER TABLE charts
                ADD COLUMN last_updated_at timestamp;

            UPDATE charts SET last_updated_at = (
                SELECT max(date) FROM chart_data
                WHERE charts.id = chart_id
                GROUP BY chart_id);

            ALTER TABLE charts
                ALTER COLUMN created_at TYPE timestamptz USING created_at
                , ALTER COLUMN created_at SET DEFAULT (now() at time zone 'utc');
            ALTER TABLE chart_data
                ALTER COLUMN created_at TYPE timestamptz USING created_at
                , ALTER COLUMN created_at SET DEFAULT (now() at time zone 'utc');
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // appropriate timezone (local) should be considered automatically;
        // local is assumed because there was no mention of timezone in init migration
        let sql = r#"
            ALTER TABLE charts
                DROP COLUMN last_updated_at;

            ALTER TABLE charts
                ALTER COLUMN created_at TYPE timestamp USING created_at
                , ALTER COLUMN created_at SET DEFAULT (now());
            ALTER TABLE chart_data
                ALTER COLUMN created_at TYPE timestamp USING created_at
                , ALTER COLUMN created_at SET DEFAULT (now());
        "#;
        crate::from_sql(manager, sql).await
    }
}
