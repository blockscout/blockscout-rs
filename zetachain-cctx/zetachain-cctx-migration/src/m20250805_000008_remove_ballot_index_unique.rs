use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Remove the unique constraint from BallotIndex column in inbound_params table
        // Since the column was defined with .unique_key(), we need to alter the column
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"ALTER TABLE inbound_params DROP CONSTRAINT IF EXISTS inbound_params_ballot_index_key;"#,
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Re-add the unique constraint to BallotIndex column in inbound_params table
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"ALTER TABLE inbound_params ADD CONSTRAINT inbound_params_ballot_index_key UNIQUE (ballot_index);"#,
        )
        .await?;

        Ok(())
    }
}
