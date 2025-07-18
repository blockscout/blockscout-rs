pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20220101_000002_add_foreign_key_indexes;
mod m20220101_000003_create_token_table;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20220101_000002_add_foreign_key_indexes::Migration),
            Box::new(m20220101_000003_create_token_table::Migration),
        ]
    }
}
