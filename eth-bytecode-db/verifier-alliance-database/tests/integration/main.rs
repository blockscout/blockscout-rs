mod contract_deployments;
mod internal_compiled_contracts;
mod transformations;
mod transformations_types;
mod verified_contracts;

macro_rules! database {
    () => {{
        let database_name = format!("{}_{}_{}", file!(), line!(), column!());
        blockscout_service_launcher::test_database::TestDbGuard::new::<
            verifier_alliance_migration_v1::Migrator,
        >(&database_name)
        .await
    }};
    ($custom_suffix:expr) => {{
        let database_name = format!("{}_{}_{}_{}", file!(), line!(), column!(), $custom_suffix);
        blockscout_service_launcher::test_database::TestDbGuard::new::<
            verifier_alliance_migration_v1::Migrator,
        >(&database_name)
        .await
    }};
}
pub(crate) use database;
