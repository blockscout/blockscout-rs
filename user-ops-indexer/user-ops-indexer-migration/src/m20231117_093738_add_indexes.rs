use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            CREATE INDEX user_operations_sender_factory_index ON user_operations (sender, factory NULLS LAST);

            CREATE INDEX user_operations_block_number_hash_index ON user_operations (block_number DESC, hash DESC);

            CREATE INDEX user_operations_block_number_transaction_hash_bundle_index_index ON user_operations (block_number DESC, transaction_hash DESC, bundle_index DESC);

            CREATE INDEX user_operations_factory_index ON user_operations (factory);

            CREATE INDEX user_operations_bundler_transaction_hash_bundle_index_index ON user_operations (bundler, transaction_hash, bundle_index);

            CREATE INDEX user_operations_paymaster_index ON user_operations (paymaster);
        "#;
        crate::from_sql(manager, sql).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let sql = r#"
            DROP INDEX user_operations_sender_factory_index;

            DROP INDEX user_operations_block_number_hash_index;

            DROP INDEX user_operations_block_number_transaction_hash_bundle_index_index;

            DROP INDEX user_operations_factory_index;

            DROP INDEX user_operations_bundler_transaction_hash_bundle_index_index;

            DROP INDEX user_operations_paymaster_index;
        "#;
        crate::from_sql(manager, sql).await
    }
}
