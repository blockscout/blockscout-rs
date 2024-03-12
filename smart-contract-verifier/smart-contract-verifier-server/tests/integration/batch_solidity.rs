use crate::{
    test_success,
    types::{
        batch_solidity::{CompilationFailure, ContractVerificationSuccess, StandardJson},
        from_file,
    },
};

const TEST_CASES_DIR: &str = "tests/test_cases_batch_solidity";

#[tokio::test]
async fn basic() {
    let (test_case_request, test_case_response) =
        from_file::<StandardJson, ContractVerificationSuccess>(TEST_CASES_DIR, "basic");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn compilation_error() {
    let (test_case_request, test_case_response) =
        from_file::<StandardJson, CompilationFailure>(TEST_CASES_DIR, "compilation_error");
    test_success(&test_case_request, &test_case_response).await;
}

#[tokio::test]
async fn invalid_standard_json() {
    let (test_case_request, test_case_response) =
        from_file::<StandardJson, CompilationFailure>(TEST_CASES_DIR, "invalid_standard_json");
    test_success(&test_case_request, &test_case_response).await;
}
