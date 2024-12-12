use crate::database;
use crate::transformations_types::TestCase;
use std::path::PathBuf;
use verifier_alliance_database::{insert_contract_deployment, insert_verified_contract};

macro_rules! build_test {
    ($test_name:ident) => {
        #[tokio::test]
        async fn $test_name() {
            const TEST_CASE_CONTENT: &str = include_str!(std::concat!("../test_cases/", stringify!($test_name), ".json"));

            let database_guard = database!();
            let database_connection = database_guard.client();

            let test_case = TestCase::from_content(TEST_CASE_CONTENT);

            let contract_deployment_data = test_case.contract_deployment_data();
            let inserted_contract_deployment =
                insert_contract_deployment(&database_connection, contract_deployment_data)
                    .await
                    .expect("error while inserting contract deployment");

            let verified_contract_data = test_case.verified_contract_data(inserted_contract_deployment.id);
            let _inserted_verified_contract =
                insert_verified_contract(&database_connection, verified_contract_data)
                    .await
                    .expect("error while inserting verified contract");

            test_case
                .validate_final_database_state(&database_connection)
                .await;
        }
    };
}

build_test!(constructor_arguments);
build_test!(full_match);
build_test!(immutables);
build_test!(libraries_linked_by_compiler);
build_test!(libraries_manually_linked);
build_test!(metadata_hash_absent);
build_test!(partial_match);
build_test!(partial_match_double_auxdata);