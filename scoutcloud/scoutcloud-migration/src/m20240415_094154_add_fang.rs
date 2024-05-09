use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, std::include_str!("fang/up.sql")).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        crate::from_sql(manager, std::include_str!("fang/down.sql")).await
    }
}
