use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql_stmts = [
            r#"
                CREATE OR REPLACE FUNCTION update_parts_data_text()
                    RETURNS TRIGGER AS
                $$
                BEGIN
                    IF NEW.data_text IS NULL THEN
                        NEW.data_text := ENCODE(NEW.data, 'hex');
                    END IF;
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;
            "#,
            r#"
                CREATE TRIGGER update_parts_data_text_trigger
                    BEFORE INSERT OR UPDATE
                    ON parts
                    FOR EACH ROW
                EXECUTE FUNCTION update_parts_data_text();
            "#,
        ];
        crate::exec_stmts(manager, sql_stmts).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TRIGGER IF EXISTS update_parts_data_text_trigger ON parts;
            DROP FUNCTION update_parts_data_text;
        "#;
        crate::from_sql(manager, sql).await
    }
}
