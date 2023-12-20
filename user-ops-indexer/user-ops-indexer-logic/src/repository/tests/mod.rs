use blockscout_service_launcher::test_database::TestDbGuard;
use migration::{
    from_sql, Alias, DynIden, IntoIden, MigrationName, MigrationTrait, MigratorTrait, SchemaManager,
};
use sea_orm::{Database, DatabaseConnection, DbErr};
use std::sync::Arc;
use tokio::sync::Mutex;

static SHARED_DB_MUTEX: Mutex<Option<String>> = Mutex::const_new(None);
pub async fn get_shared_db() -> Arc<DatabaseConnection> {
    let mut initialized = SHARED_DB_MUTEX.lock().await;
    match initialized.clone() {
        None => {
            let client = TestDbGuard::new::<TestMigrator>("shared").await;
            *initialized = Some(client.db_url());
            client.client()
        }
        Some(db_url) => Arc::new(Database::connect(db_url).await.unwrap()),
    }
}

pub struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let before: Vec<Box<dyn MigrationTrait>> = vec![Box::new(TestMigrationBefore)];
        let after: Vec<Box<dyn MigrationTrait>> = vec![Box::new(TestMigrationAfter)];
        before
            .into_iter()
            .chain(migration::Migrator::migrations())
            .chain(after)
            .collect()
    }
    fn migration_table_name() -> DynIden {
        Alias::new("user_ops_indexer_migrations").into_iden()
    }
}

pub struct TestMigrationBefore;
pub struct TestMigrationAfter;

impl MigrationName for TestMigrationBefore {
    fn name(&self) -> &str {
        "test_migration_before_0"
    }
}

impl MigrationName for TestMigrationAfter {
    fn name(&self) -> &str {
        "test_migration_after_0"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for TestMigrationBefore {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        from_sql(manager, include_str!("blockscout_tables.sql")).await
    }
}

#[async_trait::async_trait]
impl MigrationTrait for TestMigrationAfter {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        from_sql(manager, include_str!("fixtures.sql")).await
    }
}
