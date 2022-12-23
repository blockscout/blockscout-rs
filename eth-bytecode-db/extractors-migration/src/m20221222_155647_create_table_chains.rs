use sea_orm_migration::{prelude::*, sea_query::SimpleExpr};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Chains::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Chains::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Chains::CreatedAt)
                            .timestamp()
                            .not_null()
                            .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
                    )
                    .col(
                        ColumnDef::new(Chains::UpdatedAt)
                            .timestamp()
                            .not_null()
                            .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
                    )
                    .col(ColumnDef::new(Chains::PreviousRun).timestamp())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Chains::Table).to_owned())
            .await
    }
}

/// Learn more at https://docs.rs/sea-query#iden
#[derive(Iden)]
enum Chains {
    Table,
    Id,
    CreatedAt,
    UpdatedAt,
    PreviousRun,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_test() {
        let table = Table::create()
            .table(Chains::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(Chains::Id)
                    .big_integer()
                    .not_null()
                    .auto_increment()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(Chains::CreatedAt)
                    .timestamp()
                    .not_null()
                    .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
            )
            .col(
                ColumnDef::new(Chains::UpdatedAt)
                    .timestamp()
                    .not_null()
                    .default(SimpleExpr::Custom("CURRENT_TIMESTAMP".into())),
            )
            .col(ColumnDef::new(Chains::PreviousRun).timestamp())
            .to_string(SqliteQueryBuilder);
        println!("{}", table);
    }

    #[test]
    fn drop_test() {
        let table = Table::drop()
            .table(Chains::Table)
            .to_string(PostgresQueryBuilder);
        println!("{}", table);
    }
}
