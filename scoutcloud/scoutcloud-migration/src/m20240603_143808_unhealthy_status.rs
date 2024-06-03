use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            r#"
            ALTER TYPE "deployment_status_type" ADD VALUE 'unhealthy';
            "#,
        )
        .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        unimplemented!("This migration is irreversible.")
    }
}
