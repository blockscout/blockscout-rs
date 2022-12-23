use crate::m20221222_155714_create_table_bytecode_types::BytecodeTypes;
use sea_orm_migration::{
    prelude::*,
    sea_orm::{entity::prelude::*, ActiveValue, TransactionTrait},
    seaql_migrations::Relation,
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let txn = manager.get_connection().begin().await?;
        let creation_input = ActiveModel {
            bytecode_type: ActiveValue::Set("CREATION_INPUT".into()),
            seq: ActiveValue::Set(0),
        };
        let deployed_bytecode = ActiveModel {
            bytecode_type: ActiveValue::Set("DEPLOYED_BYTECODE".into()),
            seq: ActiveValue::Set(1),
        };

        Entity::insert_many([creation_input, deployed_bytecode])
            .on_conflict(
                OnConflict::column(BytecodeTypes::BytecodeType)
                    .do_nothing()
                    .to_owned(),
            )
            .exec(&txn)
            .await?;

        txn.commit().await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .truncate_table(Table::truncate().table(BytecodeTypes::Table).to_owned())
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
