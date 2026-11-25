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
            CREATE INDEX IF NOT EXISTS idx_ip_observed_hash_inc
            ON inbound_params (observed_hash) INCLUDE (cross_chain_tx_id);
            "#,
        )
        .await?;

        // Covering index for cross_chain_tx_id → index-only for joins + selected cols
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_ip_cctxid_inc
            ON inbound_params (cross_chain_tx_id)
            INCLUDE (sender_chain_id, coin_type, asset, amount, observed_hash);
            "#,
        )
        .await?;

        // Optional: covering index for status table if you join it in same query
        db.execute_unprepared(
            r#"
            CREATE INDEX IF NOT EXISTS idx_cs_cctxid_inc
            ON cctx_status (cross_chain_tx_id) INCLUDE (status, created_timestamp);
            "#,
        )
        .await?;

        // Optional cleanup: drop older non-covering indexes to reduce bloat
        db.execute_unprepared(
            r#"
            DROP INDEX IF EXISTS idx_inbound_params_observed_hash;
            DROP INDEX IF EXISTS idx_inbound_params_cross_chain_tx_id;
            -- DROP INDEX IF EXISTS idx_cctx_status_cross_chain_tx_id; -- only if it exists and is non-covering
            "#,
        ).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            DROP INDEX IF EXISTS idx_ip_observed_hash_inc;
            DROP INDEX IF EXISTS idx_ip_cctxid_inc;
            DROP INDEX IF EXISTS idx_cs_cctxid_inc;
            "#,
        )
        .await?;
        Ok(())
    }
}
