use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Covering index for observed_hash → returns root ids via index-only
        db.execute_unprepared(
            r#"
            CREATE  INDEX  IF NOT EXISTS idx_cs_last_update_txid 
            ON cctx_status (last_update_timestamp DESC, cross_chain_tx_id);
            "#,
        ).await?;

        // Covering index for cross_chain_tx_id → index-only for joins + selected cols
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_cctx_unlocked_due
             ON cross_chain_tx (last_status_update_timestamp, retries_number) WHERE processing_status = 'Unlocked';
            "#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            DROP INDEX IF EXISTS idx_cs_last_update_txid;
            DROP INDEX IF EXISTS idx_cctx_unlocked_due;
            "#,
        )
        .await?;
        Ok(())
    }
}
