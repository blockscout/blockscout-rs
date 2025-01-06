pub use sea_orm_migration::prelude::*;

mod m20241028_143125_initialize_schema_v1;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20241028_143125_initialize_schema_v1::Migration)]
    }
}
