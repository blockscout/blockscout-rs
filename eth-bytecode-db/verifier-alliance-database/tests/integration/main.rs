mod internal_code;
mod internal_compiled_contracts;
mod internal_contract_deployments;
mod internal_contracts;
mod internal_verified_contracts;

macro_rules! database {
    () => {{
        let database_name = format!("{MOD_NAME}_{TEST_NAME}");
        blockscout_service_launcher::test_database::TestDbGuard::new::<
            verifier_alliance_migration::Migrator,
        >(&database_name)
        .await
    }};
}
pub(crate) use database;
