use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let set_modified_at = r#"
            CREATE OR REPLACE FUNCTION set_modified_at()
                RETURNS TRIGGER AS
            $$
            BEGIN
                NEW.modified_at = now();
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        "#;

        crate::from_statements(manager, &[set_modified_at]).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let set_modified_at = r#"DROP FUNCTION "set_modified_at";"#;

        crate::from_statements(manager, &[set_modified_at]).await
    }
}
