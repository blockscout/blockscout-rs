use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add composite unique constraint on inbound_params table
        manager
            .create_index(
                Index::create()
                    .name("idx_inbound_params_ballot_observed_unique")
                    .table(InboundParams::Table)
                    .col(InboundParams::BallotIndex)
                    .col(InboundParams::ObservedHash)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the composite unique index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_inbound_params_ballot_observed_unique")
                    .table(InboundParams::Table)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(DeriveIden)]
enum InboundParams {
    Table,
    BallotIndex,
    ObservedHash,
}
