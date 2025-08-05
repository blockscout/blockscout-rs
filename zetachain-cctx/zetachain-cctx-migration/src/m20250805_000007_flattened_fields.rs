use sea_orm_migration::{async_trait, prelude::*, sea_orm::DeriveIden, MigrationTrait, SchemaManager};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CrossChainTx::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new("token_id").integer().null().to_owned(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new("receiver_chain_id")
                            .integer()
                            .null()
                            .to_owned(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new("receiver").string().null().to_owned(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_token_id")
                    .from(CrossChainTx::Table, CrossChainTx::TokenId)
                    .to(Token::Table, Token::Id)
                    .to_owned()
                    .on_delete(ForeignKeyAction::Cascade)
                    .on_update(ForeignKeyAction::Cascade)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.alter_table(Table::alter().table(CrossChainTx::Table).drop_column(CrossChainTx::TokenId).to_owned()).await?;
        manager.alter_table(Table::alter().table(CrossChainTx::Table).drop_column(CrossChainTx::ReceiverChainId).to_owned()).await?;
        manager.alter_table(Table::alter().table(CrossChainTx::Table).drop_column(CrossChainTx::Receiver).to_owned()).await?;
        Ok(())
    }
}
#[derive(DeriveIden)]
enum CrossChainTx {
    Table,
    TokenId,
    ReceiverChainId,
    Receiver,
}
#[derive(DeriveIden)]
enum Token {
    Table,
    Id,
}