use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX user_operations_sender_factory_index ON user_operations (sender, factory NULLS LAST);

            CREATE INDEX user_operations_block_number_op_hash_index ON user_operations (block_number DESC, op_hash DESC);

            CREATE INDEX user_operations_block_number_tx_hash_bundle_index_index ON user_operations (block_number DESC, tx_hash DESC, bundle_index DESC);

            CREATE INDEX user_operations_factory_index ON user_operations (factory);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX user_operations_sender_factory_index;

            DROP INDEX user_operations_block_number_op_hash_index;

            DROP INDEX user_operations_block_number_tx_hash_bundle_index_index;

            DROP INDEX user_operations_factory_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
