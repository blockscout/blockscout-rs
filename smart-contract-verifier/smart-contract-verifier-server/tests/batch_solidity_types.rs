use std::collections::BTreeMap;
use pretty_assertions::assert_eq;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    self as proto, batch_verify_response, BatchVerifyResponse,
    BatchVerifySolidityStandardJsonRequest,
};

const TEST_CASES_DIR: &str = "tests/test_cases_batch_solidity";

pub trait TestCaseRequest {
    fn route() -> &'static str;

    fn to_request(&self) -> Value;
}

pub trait TestCaseResponse {
    fn check(&self, actual_response: BatchVerifyResponse) -> ();
}

pub fn from_file<Request, Response>(test_case: &str) -> (Request, Response)
where
    Request: TestCaseRequest + DeserializeOwned,
    Response: TestCaseResponse + DeserializeOwned,
{
    let test_case_path = format!("{TEST_CASES_DIR}/{test_case}.json");
    let content = std::fs::read_to_string(test_case_path).expect("failed to read file");

    let request: Request =
        serde_json::from_str(&content).expect("invalid test case request format");
    let response: Response =
        serde_json::from_str(&content).expect("invalid test case response format");
    (request, response)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StandardJson {
    request: BatchVerifySolidityStandardJsonRequest,
}

impl TestCaseRequest for StandardJson {
    fn route() -> &'static str {
        "/api/v2/verifier/solidity/sources:batch-verify-standard-json"
    }

    fn to_request(&self) -> Value {
        serde_json::to_value(self.request.clone()).expect("request serialization failed")
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Response<T> {
    response: T
}

impl<T: TestCaseResponse> TestCaseResponse for Response<T> {
    fn check(&self, actual_response: BatchVerifyResponse) -> () {
        self.response.check(actual_response)
    }
}

pub type CompilationFailure = Response<compilation_failure::CompilationFailure>;
mod compilation_failure {
    use serde::{Deserialize, Deserializer};
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{self as proto, batch_verify_response, BatchVerifyResponse};
    use crate::TestCaseResponse;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all="camelCase")]
    pub struct CompilationFailure {
        compilation_failure: CompilationFailureInternal,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all="camelCase")]
    struct CompilationFailureInternal {
        message: String,
    }

    impl TestCaseResponse for CompilationFailure {
        fn check(&self, actual_response: BatchVerifyResponse) -> () {
            let result = actual_response
                .verification_result
                .expect("verification result is missing from response");
            match result {
                batch_verify_response::VerificationResult::CompilationFailure(
                    proto::CompilationFailure {
                        message: actual_message,
                    },
                ) => {
                    if !actual_message.contains(&self.compilation_failure.message) {
                        panic!(
                            "invalid compilation failure message; expected={}, actual={actual_message}",
                            self.compilation_failure.message
                        )
                    }
                }
                result => panic!(
                    "invalid verification result; expected CompilationFailure, actual={result:?}"
                ),
            }
        }
    }
}

pub type ContractVerificationSuccess = Response<contract_verification_success::ContractVerificationSuccess>;
mod contract_verification_success {
    use super::TestCaseResponse;
    use std::collections::BTreeMap;
    use serde::Deserialize;
    use serde_json::Value;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::BatchVerifyResponse;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all="camelCase")]
    pub struct ContractVerificationSuccess {
        pub success: ContractVerificationSuccessInternal,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all="camelCase")]
    pub struct ContractVerificationSuccessInternal {
        pub creation_code: String,
        pub runtime_code: String,
        pub compiler: String,
        pub compiler_version: String,
        pub language: String,
        pub file_name: String,
        pub contract_name: String,
        pub sources: BTreeMap<String, String>,
        pub compiler_settings: Value,
        pub compilation_artifacts: Value,
        pub creation_code_artifacts: Value,
        pub runtime_code_artifacts: Value,
        pub creation_match: bool,
        pub creation_values: Value,
        pub creation_transformations: Value,
        pub runtime_match: bool,
        pub runtime_values: Value,
        pub runtime_transformations: Value,
    }

    impl TestCaseResponse for ContractVerificationSuccess {
        fn check(&self, actual_response: BatchVerifyResponse) -> () {
            // let result = actual_response
            //     .verification_result
            //     .expect("verification result is missing from response");
            // match result {
            //     batch_verify_response::VerificationResult::CompilationFailure(
            //         proto::CompilationFailure {
            //             message: actual_message,
            //         },
            //     ) => {
            //         if !actual_message.contains(&self.0.message) {
            //             panic!(
            //                 "invalid compilation failure message; expected={}, actual={actual_message}",
            //                 self.0.message
            //             )
            //         }
            //     }
            //     result => panic!(
            //         "invalid verification result; expected CompilationFailure, actual={result:?}"
            //     ),
            // }
        }
    }
}
