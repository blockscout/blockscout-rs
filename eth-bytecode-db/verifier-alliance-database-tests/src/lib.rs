mod test_case;

pub use blockscout_service_launcher::test_database::TestDbGuard;
pub use test_case::TestCase;

pub use paste;

/*************** public test cases ***************/

use blockscout_service_launcher::database;
use std::future::Future;
use verifier_alliance_migration_v1::Migrator;

pub async fn test<F, Fut>(test_case: TestCase, initialization: F)
where
    F: FnOnce(TestDbGuard, TestCase) -> Fut,
    Fut: Future<Output = ()>,
{
    let database_guard = database!(Migrator, &test_case.test_case_name);

    initialization(database_guard.clone(), test_case.clone()).await;

    test_case
        .validate_final_database_state(&database_guard.client())
        .await;
}

macro_rules! build_test_case {
    ($test_name:ident) => {
        $crate::paste::paste! {
            pub fn [<$test_name _test_case>]() -> TestCase {
                const TEST_CASE_CONTENT: &str =
                    include_str!(concat!("../test_cases/", stringify!($test_name), ".json"));
                TestCase::from_content(stringify!($test_name), TEST_CASE_CONTENT)
            }
        }
    };
}

build_test_case!(constructor_arguments);
build_test_case!(full_match);
build_test_case!(immutables);
build_test_case!(libraries_linked_by_compiler);
build_test_case!(libraries_manually_linked);
build_test_case!(metadata_hash_absent);
build_test_case!(partial_match);
build_test_case!(partial_match_double_auxdata);

#[macro_export]
macro_rules! build_test {
    ($test_name:ident, $initialization:ident) => {
        // #[test_log::test(tokio::test)]
        #[tokio::test]
        pub async fn $test_name() {
            $crate::paste::paste! {
                let test_case = $crate::[<$test_name _test_case>]();
            }
            $crate::test(test_case, $initialization).await;
        }
    };
}

#[macro_export]
macro_rules! build_all_tests {
    ($initialization:ident) => {
        $crate::build_all_tests!(
            (
                constructor_arguments,
                full_match,
                immutables,
                libraries_linked_by_compiler,
                libraries_manually_linked,
                metadata_hash_absent,
                partial_match,
                partial_match_double_auxdata
            ),
            $initialization
        );
    };
    (($($test_name:ident),+), $initialization:ident) => {
        $(
            $crate::build_test!($test_name, $initialization);

        )+
    };
}
