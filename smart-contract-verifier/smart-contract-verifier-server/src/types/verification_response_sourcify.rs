use serde::{Deserialize, Serialize};
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyViaSourcifyResponse;
use std::{fmt::Display, ops::Deref};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct VerifyViaSourcifyResponseWrapper(VerifyViaSourcifyResponse);

impl From<VerifyViaSourcifyResponse> for VerifyViaSourcifyResponseWrapper {
    fn from(inner: VerifyViaSourcifyResponse) -> Self {
        Self(inner)
    }
}

impl Deref for VerifyViaSourcifyResponseWrapper {
    type Target = VerifyViaSourcifyResponse;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl VerifyViaSourcifyResponseWrapper {
    pub fn into_inner(self) -> VerifyViaSourcifyResponse {
        self.0
    }
}

impl VerifyViaSourcifyResponseWrapper {
    pub fn ok(result: verify_via_sourcify_response::ResultWrapper) -> Self {
        VerifyViaSourcifyResponse {
            message: "OK".to_string(),
            status: "0".to_string(),
            result: Some(result.into_inner()),
        }
        .into()
    }

    pub fn err(message: impl Display) -> Self {
        VerifyViaSourcifyResponse {
            message: message.to_string(),
            status: "1".to_string(),
            result: None,
        }
        .into()
    }
}

pub mod verify_via_sourcify_response {
    use std::ops::Deref;
    use smart_contract_verifier::{SourcifySuccess};
    pub use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::verify_via_sourcify_response::Result;
    use serde::{Serialize, Deserialize};
    use blockscout_display_bytes::Bytes as DisplayBytes;

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    pub struct ResultWrapper(Result);

    impl From<Result> for ResultWrapper {
        fn from(inner: Result) -> Self {
            Self(inner)
        }
    }

    impl Deref for ResultWrapper {
        type Target = Result;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl ResultWrapper {
        pub fn into_inner(self) -> Result {
            self.0
        }
    }

    impl From<SourcifySuccess> for ResultWrapper {
        fn from(value: SourcifySuccess) -> Self {
            let inner = Result {
                file_name: value.file_name,
                contract_name: value.contract_name,
                compiler_version: value.compiler_version,
                sources: value.sources,
                evm_version: value.evm_version,
                optimization: value.optimization,
                optimization_runs: value.optimization_runs.map(|i| i as i32),
                contract_libraries: value.contract_libraries,
                compiler_settings: value.compiler_settings,
                constructor_arguments: value
                    .constructor_arguments
                    .map(|bytes| DisplayBytes::from(bytes).to_string()),
                abi: Some(value.abi),

                match_type: result::MatchTypeWrapper::from(value.match_type)
                    .into_inner()
                    .into(),
            };

            inner.into()
        }
    }

    pub mod result {
        use std::ops::Deref;
        pub use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::verify_via_sourcify_response::result::{MatchType};
        use serde::{Serialize, Deserialize};

        #[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
        pub struct MatchTypeWrapper(MatchType);

        impl From<MatchType> for MatchTypeWrapper {
            fn from(inner: MatchType) -> Self {
                Self(inner)
            }
        }

        impl Deref for MatchTypeWrapper {
            type Target = MatchType;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl MatchTypeWrapper {
            pub fn into_inner(self) -> MatchType {
                self.0
            }
        }

        impl From<smart_contract_verifier::MatchType> for MatchTypeWrapper {
            fn from(value: smart_contract_verifier::MatchType) -> Self {
                let inner = match value {
                    smart_contract_verifier::MatchType::Partial => MatchType::Partial,
                    smart_contract_verifier::MatchType::Full => MatchType::Full,
                };
                inner.into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        verify_via_sourcify_response::{Result, ResultWrapper},
        *,
    };
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use pretty_assertions::assert_eq;
    use smart_contract_verifier::{MatchType, SourcifySuccess};
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v1::VerifyViaSourcifyResponse;
    use std::{collections::BTreeMap, str::FromStr};

    #[test]
    fn from_sourcify_success() {
        let verification_success = SourcifySuccess {
            file_name: "file_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            evm_version: "london".to_string(),
            optimization: Some(true),
            optimization_runs: Some(200),
            constructor_arguments: Some(DisplayBytes::from_str("0x123456").unwrap().0),
            contract_name: "contract_name".to_string(),
            abi: "abi".to_string(),
            sources: BTreeMap::from([("path".into(), "content".into())]),
            contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
            compiler_settings: "compiler_settings".to_string(),
            match_type: MatchType::Full,
        };
        let result = ResultWrapper::from(verification_success).into_inner();

        let expected = Result {
            file_name: "file_name".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            sources: BTreeMap::from([("path".into(), "content".into())]),
            evm_version: "london".to_string(),
            optimization: Some(true),
            optimization_runs: Some(200),
            contract_libraries: BTreeMap::from([("lib_name".into(), "lib_address".into())]),
            compiler_settings: "compiler_settings".to_string(),
            constructor_arguments: Some("0x123456".into()),
            abi: Some("abi".to_string()),
            match_type: 2,
        };

        assert_eq!(expected, result);
    }

    #[test]
    fn ok_verify_response() {
        let verification_success = SourcifySuccess {
            file_name: "file_path".to_string(),
            contract_name: "contract_name".to_string(),
            compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
            evm_version: "london".to_string(),
            optimization: None,
            optimization_runs: None,
            constructor_arguments: None,
            contract_libraries: Default::default(),
            abi: "abi".to_string(),
            sources: Default::default(),
            compiler_settings: "compiler_settings".to_string(),
            match_type: MatchType::Full,
        };
        let result = ResultWrapper::from(verification_success);

        let response = VerifyViaSourcifyResponseWrapper::ok(result.clone()).into_inner();

        let expected = VerifyViaSourcifyResponse {
            message: "OK".to_string(),
            status: "0".to_string(),
            result: Some(result.into_inner()),
        };

        assert_eq!(expected, response);
    }

    #[test]
    fn err_verify_response() {
        let response = VerifyViaSourcifyResponseWrapper::err("parse error").into_inner();
        let expected = VerifyViaSourcifyResponse {
            message: "parse error".to_string(),
            status: "1".to_string(),
            result: None,
        };
        assert_eq!(expected, response);
    }
}
