use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!("migrations_up/m20260720_120000_add_read_filters_and_bridge_stats_up.sql"),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!(
                "migrations_down/m20260720_120000_add_read_filters_and_bridge_stats_down.sql"
            ),
        )
        .await?;
        Ok(())
    }
}
