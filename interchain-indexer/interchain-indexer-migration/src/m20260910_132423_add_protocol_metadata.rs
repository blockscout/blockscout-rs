use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!("migrations_up/m20260910_132423_add_protocol_metadata_up.sql"),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!("migrations_down/m20260910_132423_add_protocol_metadata_down.sql"),
        )
        .await?;
        Ok(())
    }
}
