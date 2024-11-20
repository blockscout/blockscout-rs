mod internal_compiled_contracts;
mod contract_deployments;
mod verified_contracts;
mod transformations_types;
mod transformations;

macro_rules! database {
    () => {{
        let database_name = format!("{}_{}_{}", file!(), line!(), column!());
        blockscout_service_launcher::test_database::TestDbGuard::new::<
            verifier_alliance_migration::Migrator,
        >(&database_name)
        .await
    }};
    ($custom_suffix:expr) => {{
        let database_name = format!("{}_{}_{}_{}", file!(), line!(), column!(), $custom_suffix);
        blockscout_service_launcher::test_database::TestDbGuard::new::<
            verifier_alliance_migration::Migrator,
        >(&database_name)
        .await
    }};
}
pub(crate) use database;
