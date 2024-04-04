use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // probably having string limits is better
        manager
            .create_table(
                Table::create()
                    .table(ContractUrl::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContractUrl::ChainId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContractUrl::Address)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ContractUrl::Url).string_len(512).not_null())
                    .primary_key(
                        Index::create()
                            .col(ContractSources::ChainId)
                            .col(ContractSources::Address),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ContractSources::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ContractSources::ChainId)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContractSources::Address)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContractSources::Filename)
                            .string_len(32768)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ContractSources::Contents)
                            .string()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(ContractSources::ChainId)
                            .col(ContractSources::Address)
                            .col(ContractSources::Filename), // shouldn't have duplicates
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ContractUrl::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ContractSources::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum ContractUrl {
    Table,
    ChainId,
    Address,
    Url,
}

#[derive(DeriveIden)]
enum ContractSources {
    Table,
    ChainId,
    Address,
    Filename,
    Contents,
}
