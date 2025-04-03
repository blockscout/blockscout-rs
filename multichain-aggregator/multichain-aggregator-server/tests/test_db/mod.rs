use migration::{
    from_sql, Alias, DynIden, IntoIden, MigrationName, MigrationTrait, MigratorTrait, SchemaManager,
};
use sea_orm::DbErr;

pub struct TestMigrator;

#[async_trait::async_trait]
impl MigratorTrait for TestMigrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        let mut migrations = migration::Migrator::migrations();
        migrations.push(Box::new(FixturesMigration));
        migrations
    }

    fn migration_table_name() -> DynIden {
        Alias::new("test_fixtures_migration").into_iden()
    }
}

struct FixturesMigration;

impl MigrationName for FixturesMigration {
    fn name(&self) -> &str {
        "fixtures_migration"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for FixturesMigration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        from_sql(manager, include_str!("fixtures.sql")).await
    }
}
