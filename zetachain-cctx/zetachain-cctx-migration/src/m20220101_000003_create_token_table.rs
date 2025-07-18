use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Token::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Token::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Token::Zrc20ContractAddress)
                            .string()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Token::Asset).string().not_null())
                    .col(ColumnDef::new(Token::ForeignChainId).string().not_null())
                    .col(ColumnDef::new(Token::Decimals).integer().not_null())
                    .col(ColumnDef::new(Token::Name).string().not_null())
                    .col(ColumnDef::new(Token::Symbol).string().not_null())
                    .col(ColumnDef::new(Token::CoinType).enumeration("coin_type", ["Zeta", "Gas", "Erc20", "Cmd", "NoAssetCall"]).not_null())
                    .col(ColumnDef::new(Token::GasLimit).string().not_null())
                    .col(ColumnDef::new(Token::Paused).boolean().not_null().default(false))
                    .col(ColumnDef::new(Token::LiquidityCap).string().not_null())
                    .col(
                        ColumnDef::new(Token::CreatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Token::UpdatedAt)
                            .date_time()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on asset for fast lookups
        manager
            .create_index(
                Index::create()
                    .name("idx_token_asset")
                    .table(Token::Table)
                    .col(Token::Asset)
                    .to_owned(),
            )
            .await?;

        // Create index on foreign_chain_id for filtering
        manager
            .create_index(
                Index::create()
                    .name("idx_token_foreign_chain_id")
                    .table(Token::Table)
                    .col(Token::ForeignChainId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_token_asset")
                    .table(Token::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_token_foreign_chain_id")
                    .table(Token::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(Token::Table).to_owned())
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Token {
    Table,
    Id,
    Zrc20ContractAddress,
    Asset,
    ForeignChainId,
    Decimals,
    Name,
    Symbol,
    CoinType,
    GasLimit,
    Paused,
    LiquidityCap,
    CreatedAt,
    UpdatedAt,
} 