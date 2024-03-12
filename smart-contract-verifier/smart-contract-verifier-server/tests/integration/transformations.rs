use crate::{
    test_success,
    types::{from_file, transformations::TestCase},
};

const TEST_CASES_DIR: &str = "tests/test_cases_solidity_transformations";

#[tokio::test]
async fn constructor_arguments() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "constructor_arguments");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn full_match() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "full_match");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn immutables() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "immutables");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn libraries_linked_by_compiler() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "libraries_linked_by_compiler");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn libraries_manually_linked() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "libraries_manually_linked");
    test_success(&test_case_request, &test_case_response).await;
}

// TODO: no auxdata is parsed right now when `metadataHash` is "none"
// #[tokio::test]
// async fn metadata_hash_absent() {
//     let (test_case_request, test_case_response) =
//         from_file::<TestCase, TestCase>(TEST_CASES_DIR, "metadata_hash_absent");
//     test_success(&test_case_request, &test_case_response).await;
// }

#[tokio::test]
async fn partial_match() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "partial_match");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn partial_match_double_auxdata() {
    let (test_case_request, test_case_response) =
        from_file::<TestCase, TestCase>(TEST_CASES_DIR, "partial_match_double_auxdata");
    test_success(&test_case_request, &test_case_response).await;
}
