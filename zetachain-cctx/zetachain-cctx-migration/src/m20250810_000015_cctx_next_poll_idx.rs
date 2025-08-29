use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_cctx_unlocked_next_poll
            ON cross_chain_tx (next_poll, last_status_update_timestamp)
            WHERE processing_status = 'Unlocked';
            "#,
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            DROP INDEX IF EXISTS idx_cctx_unlocked_next_poll;
            "#,
        )
        .await?;
        Ok(())
    }
}


