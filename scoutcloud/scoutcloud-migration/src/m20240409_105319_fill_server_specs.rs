use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, r#"
        INSERT INTO server_specs(slug, cost_per_hour, resources_config) VALUES
        ('small', 1, '{"limits": {"memory": "4Gi", "cpu": "2"}, "requests": {"memory": "2Gi", "cpu": "1"}}'),
        ('medium', 2, '{"limits": {"memory": "8Gi", "cpu": "4"}, "requests": {"memory": "4Gi", "cpu": "2"}}'),
        ('large', 4, '{"limits": {"memory": "16Gi", "cpu": "8"}, "requests": {"memory": "8Gi", "cpu": "4"}}')
        "#).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, r#"DELETE FROM server_specs"#).await?;
        Ok(())
    }
}
