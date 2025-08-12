use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Index to accelerate lookup by parent for traversals
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cross_chain_tx_root_id_depth")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::RootId)
                    .col(CrossChainTx::Depth)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_cross_chain_tx_root_id")
                    .table(CrossChainTx::Table)
                    .col(CrossChainTx::RootId)
                    .to_owned(),
            )
            .await?;
        
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_root_id_depth")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("idx_cross_chain_tx_root_id")
                    .table(CrossChainTx::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CrossChainTx {
    Table,
    RootId,
    Depth,
}


