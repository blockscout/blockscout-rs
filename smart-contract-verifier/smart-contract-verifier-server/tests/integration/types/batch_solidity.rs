use super::*;
use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::Deserialize;
use serde_json::Value;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    self as proto, batch_verify_response, contract_verification_result, BatchVerifyResponse,
    BatchVerifySolidityStandardJsonRequest,
};
use std::{collections::BTreeMap, str::FromStr};

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
    response: T,
}

impl<Proto, T> TestCaseResponse for Response<T>
where
    Proto: for<'de> serde::Deserialize<'de>,
    T: TestCaseResponse<Response = Proto>,
{
    type Response = Proto;

    fn check(&self, actual_response: Self::Response) {
        self.response.check(actual_response)
    }
}

pub type CompilationFailure = Response<compilation_failure::CompilationFailure>;
mod compilation_failure {
    use super::*;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
        self as proto, batch_verify_response, BatchVerifyResponse,
    };

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CompilationFailure {
        compilation_failure: CompilationFailureInternal,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CompilationFailureInternal {
        message: String,
    }

    impl TestCaseResponse for CompilationFailure {
        type Response = BatchVerifyResponse;

        fn check(&self, actual_response: Self::Response) {
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

pub type ContractVerificationSuccess =
    Response<contract_verification_success::ContractVerificationSuccess>;
mod contract_verification_success {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde::Deserialize;
    use serde_json::Value;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::BatchVerifyResponse;
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ContractVerificationSuccess {
        pub success: ContractVerificationSuccessInternal,
    }

    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ContractVerificationSuccessInternal {
        pub creation_code: DisplayBytes,
        pub runtime_code: DisplayBytes,
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
        pub creation_values: Option<Value>,
        pub creation_transformations: Option<Value>,
        pub runtime_match: bool,
        pub runtime_values: Option<Value>,
        pub runtime_transformations: Option<Value>,
    }

    impl TestCaseResponse for ContractVerificationSuccess {
        type Response = BatchVerifyResponse;

        fn check(&self, actual_response: Self::Response) {
            let ParsedSuccessItem {
                creation_code,
                runtime_code,
                compiler,
                compiler_version,
                language,
                file_name,
                contract_name,
                sources,
                compiler_settings,
                compilation_artifacts,
                creation_code_artifacts,
                runtime_code_artifacts,
                creation_match,
                creation_values,
                creation_transformations,
                runtime_match,
                runtime_values,
                runtime_transformations,
            } = retrieve_success_item(actual_response);

            assert_eq!(
                self.success.creation_code, creation_code,
                "invalid creation_code"
            );
            assert_eq!(
                self.success.runtime_code, runtime_code,
                "invalid runtime_code"
            );
            assert_eq!(self.success.compiler, compiler, "invalid compiler");
            assert_eq!(
                self.success.compiler_version, compiler_version,
                "invalid compiler_version"
            );
            assert_eq!(self.success.language, language, "invalid language");
            assert_eq!(self.success.file_name, file_name, "invalid file_name");
            assert_eq!(
                self.success.contract_name, contract_name,
                "invalid contract_name"
            );
            assert_eq!(self.success.sources, sources, "invalid sources");
            assert_eq!(
                self.success.compiler_settings, compiler_settings,
                "invalid compiler_settings"
            );
            assert_eq!(
                self.success.compilation_artifacts, compilation_artifacts,
                "invalid compilation_artifacts"
            );
            assert_eq!(
                self.success.creation_code_artifacts, creation_code_artifacts,
                "invalid creation_code_artifacts"
            );
            assert_eq!(
                self.success.runtime_code_artifacts, runtime_code_artifacts,
                "invalid runtime_code_artifacts"
            );
            assert_eq!(
                self.success.creation_match, creation_match,
                "invalid creation_match"
            );
            assert_eq!(
                self.success.creation_values, creation_values,
                "invalid creation_values"
            );
            assert_eq!(
                self.success.creation_transformations, creation_transformations,
                "invalid creation_transformations"
            );
            assert_eq!(
                self.success.runtime_match, runtime_match,
                "invalid runtime_match"
            );
            assert_eq!(
                self.success.runtime_values, runtime_values,
                "invalid runtime_values"
            );
            assert_eq!(
                self.success.runtime_transformations, runtime_transformations,
                "invalid runtime_transformations"
            );
        }
    }
}

#[derive(Clone, Debug)]
pub struct ParsedSuccessItem {
    pub creation_code: DisplayBytes,
    pub runtime_code: DisplayBytes,
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
    pub creation_values: Option<Value>,
    pub creation_transformations: Option<Value>,
    pub runtime_match: bool,
    pub runtime_values: Option<Value>,
    pub runtime_transformations: Option<Value>,
}

impl From<proto::ContractVerificationSuccess> for ParsedSuccessItem {
    fn from(value: proto::ContractVerificationSuccess) -> ParsedSuccessItem {
        let proto::ContractVerificationSuccess {
            creation_code,
            runtime_code,
            compiler,
            compiler_version,
            language,
            file_name,
            contract_name,
            sources,
            compiler_settings,
            compilation_artifacts,
            creation_code_artifacts,
            runtime_code_artifacts,
            creation_match,
            creation_values,
            creation_transformations,
            runtime_match,
            runtime_values,
            runtime_transformations,
        } = value;

        let creation_code =
            DisplayBytes::from_str(&creation_code).expect("cannot parse creation_code as bytes");
        let runtime_code =
            DisplayBytes::from_str(&runtime_code).expect("cannot parse runtime_code as bytes");

        let compiler = smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::contract_verification_success::compiler::Compiler::from_i32(compiler).unwrap().as_str_name();
        let language = smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::contract_verification_success::language::Language::from_i32(language).unwrap().as_str_name();
        let compiler_settings = {
            let mut compiler_settings = serde_json::Value::from_str(&compiler_settings)
                .expect("cannot parse compiler_settings as json");
            compiler_settings
                .as_object_mut()
                .expect("compiler_settings is not an object")
                .remove("outputSelection");
            compiler_settings
        };

        let compilation_artifacts = serde_json::Value::from_str(&compilation_artifacts)
            .expect("cannot parse compilation_artifacts as json");
        let creation_code_artifacts = serde_json::Value::from_str(&creation_code_artifacts)
            .expect("cannot parse creation_code_artifacts as json");
        let runtime_code_artifacts = serde_json::Value::from_str(&runtime_code_artifacts)
            .expect("cannot parse runtime_code_artifacts as json");

        macro_rules! maybe_string_to_value {
            ($field:ident) => {
                $field.map(|v| {
                    serde_json::Value::from_str(&v)
                        .expect(&format!("cannot parse {} as json", stringify!($field)))
                })
            };
        }

        let creation_values = maybe_string_to_value!(creation_values);
        let creation_transformations = maybe_string_to_value!(creation_transformations);
        let runtime_values = maybe_string_to_value!(runtime_values);
        let runtime_transformations = maybe_string_to_value!(runtime_transformations);

        ParsedSuccessItem {
            creation_code,
            runtime_code,
            compiler: compiler.to_string(),
            compiler_version,
            language: language.to_string(),
            file_name,
            contract_name,
            sources,
            compiler_settings,
            compilation_artifacts,
            creation_code_artifacts,
            runtime_code_artifacts,
            creation_match,
            creation_values,
            creation_transformations,
            runtime_match,
            runtime_values,
            runtime_transformations,
        }
    }
}

pub fn retrieve_success_item(response: BatchVerifyResponse) -> ParsedSuccessItem {
    let result = response
        .verification_result
        .expect("verification result is missing from response");
    match result {
        batch_verify_response::VerificationResult::ContractVerificationResults(
            batch_verify_response::ContractVerificationResults { items },
        ) => {
            pretty_assertions::assert_eq!(
                1,
                items.len(),
                "only 1 contract expected inside results"
            );
            let item = items[0].clone();
            match item {
                proto::ContractVerificationResult {
                    verification_result:
                        Some(contract_verification_result::VerificationResult::Success(success)),
                } => success.into(),
                result => panic!(
                    "invalid contract verification result; expected Success, actual={result:?}"
                ),
            }
        }
        result => {
            panic!("invalid verification result; expected CompilationFailure, actual={result:?}")
        }
    }
}
