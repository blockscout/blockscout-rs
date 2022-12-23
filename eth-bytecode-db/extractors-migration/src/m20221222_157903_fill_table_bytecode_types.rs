use sea_orm_migration::sea_orm::{ActiveValue, TransactionTrait};
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::entity::prelude::*;
use sea_orm_migration::seaql_migrations::Relation;
use crate::m20221222_155714_create_table_bytecode_types::BytecodeTypes;
// use crate::m20221222_155714_create_table_bytecode_types::BytecodeTypes;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn= manager.get_connection().begin().await?;
        let _creation_input = ActiveModel {
            bytecode_type: ActiveValue::Set("CREATION_INPUT".into()),
            seq: ActiveValue::Set(0),
        }.insert(&txn).await?;
        let _deployed_bytecode= ActiveModel {
            bytecode_type: ActiveValue::Set("DEPLOYED_BYTECODE".into()),
            seq: ActiveValue::Set(1),
        }.insert(&txn).await?;

        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(
                Table::truncate().table(BytecodeTypes::Table).to_owned())
            .await
    }
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "bytecode_types")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub bytecode_type: String,
    pub seq: i32,
}

impl ActiveModelBehavior for ActiveModel {}