use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql_stmts = [
            r#"
                CREATE OR REPLACE FUNCTION update_sources_bytecode_text()
                    RETURNS TRIGGER AS
                $$
                BEGIN
                    IF NEW.raw_creation_input_text IS NULL THEN
                        NEW.raw_creation_input_text := ENCODE(NEW.raw_creation_input, 'hex');
                    END IF;
                    IF NEW.raw_deployed_bytecode_text IS NULL THEN
                        NEW.raw_deployed_bytecode_text := ENCODE(NEW.raw_deployed_bytecode, 'hex');
                    END IF;
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql;
            "#,
            r#"
                CREATE TRIGGER update_sources_bytecode_text_trigger
                    BEFORE INSERT OR UPDATE
                    ON sources
                    FOR EACH ROW
                EXECUTE FUNCTION update_sources_bytecode_text();
            "#,
        ];
        crate::exec_stmts(manager, sql_stmts).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP TRIGGER IF EXISTS update_sources_bytecode_text_trigger ON sources;
            DROP FUNCTION update_sources_bytecode_text;
        "#;
        crate::from_sql(manager, sql).await
    }
}
