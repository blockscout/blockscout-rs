pub use sea_orm_migration::prelude::*;

mod m20220101_000001_create_table;
mod m20220101_000002_add_foreign_key_indexes;
mod m20220101_000003_create_token_table;
mod m20220101_000004_add_inbound_params_composite_unique;
mod m20240101_000005_add_icon_url_to_token;
mod m20240101_000006_add_performance_indices;
mod m20250805_000007_flattened_fields;
mod m20250805_000008_remove_ballot_index_unique;
mod m20250807_000009_add_cctx_status_indices;
mod m20250807_000010_tree_indices;
mod m20250810_000011_root_id_default;
mod m20250810_000012_inbound_params_idx;
mod m20250810_000013_cctx_status_indices;
mod m20250810_000014_cctx_next_poll;
mod m20250810_000015_cctx_next_poll_idx;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20220101_000001_create_table::Migration),
            Box::new(m20220101_000002_add_foreign_key_indexes::Migration),
            Box::new(m20220101_000003_create_token_table::Migration),
            Box::new(m20220101_000004_add_inbound_params_composite_unique::Migration),
            Box::new(m20240101_000005_add_icon_url_to_token::Migration),
            Box::new(m20240101_000006_add_performance_indices::Migration),
            Box::new(m20250805_000007_flattened_fields::Migration),
            Box::new(m20250805_000008_remove_ballot_index_unique::Migration),
            Box::new(m20250807_000009_add_cctx_status_indices::Migration),
            Box::new(m20250807_000010_tree_indices::Migration),
            Box::new(m20250810_000011_root_id_default::Migration),
            Box::new(m20250810_000012_inbound_params_idx::Migration),
            Box::new(m20250810_000013_cctx_status_indices::Migration),
            Box::new(m20250810_000014_cctx_next_poll::Migration),
            Box::new(m20250810_000015_cctx_next_poll_idx::Migration),
        ]
    }
}
