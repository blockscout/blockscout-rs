use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Index to accelerate lookup by parent for traversals
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            ALTER TABLE cross_chain_tx 
            ALTER COLUMN root_id 
            SET DEFAULT currval(pg_get_serial_sequence('cross_chain_tx','id'));
            "#
        ).await?;
        
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
            ALTER TABLE cross_chain_tx 
            ALTER COLUMN root_id 
            DROP DEFAULT;
            "#
        ).await?;
        Ok(())
    }
}