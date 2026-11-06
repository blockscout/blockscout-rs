use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!(
                "migrations_up/m20261106_084608_add_message_address_search_indexes_up.sql"
            ),
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(
            manager,
            include_str!(
                "migrations_down/m20261106_084608_add_message_address_search_indexes_down.sql"
            ),
        )
        .await?;
        Ok(())
    }
}
