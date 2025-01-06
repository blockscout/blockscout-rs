use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // remove unique constrint on `name` column
        // add enumeration `chart_resolution`
        // add column `resolution` with the new type and default being `DAY`
        // add unique constraint on combination of (`name` + `resolution`) columns
        let sql = r#"
            ALTER TABLE charts
                DROP CONSTRAINT charts_name_key;
                
            CREATE TYPE "chart_resolution" AS ENUM (
                'DAY',
                'WEEK',
                'MONTH',
                'YEAR'
            );

            ALTER TABLE charts
                ADD COLUMN resolution chart_resolution NOT NULL DEFAULT 'DAY';

            ALTER TABLE charts
                ADD CONSTRAINT charts_name_resolution_key UNIQUE (name, resolution);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            ALTER TABLE charts
                DROP CONSTRAINT charts_name_resolution_key;

            ALTER TABLE charts
                DROP COLUMN resolution;

            DROP TYPE "chart_resolution";

            ALTER TABLE charts
                ADD CONSTRAINT charts_name_key UNIQUE (name);
        "#;
        crate::from_sql(manager, sql).await
    }
}
