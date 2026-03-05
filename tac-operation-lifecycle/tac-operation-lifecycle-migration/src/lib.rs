pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20250512_135947_add_operation_metainfo;
mod m20260304_204118_mark_insufficient_fee_operations;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20250512_135947_add_operation_metainfo::Migration),
            Box::new(m20260304_204118_mark_insufficient_fee_operations::Migration),
        ]
    }
}
