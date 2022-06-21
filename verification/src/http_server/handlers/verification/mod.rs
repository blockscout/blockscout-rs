#![allow(dead_code)]

use ethers_core::types::Bytes as DisplayBytes;
use ethers_solc::CompilerInput;
use std::{collections::BTreeMap, fmt::Display};

use crate::{compiler::version::CompilerVersion, solidity::VerificationSuccess};
use serde::{Deserialize, Serialize};

pub mod solidity;
pub mod sourcify;

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificationResponse {
    pub message: String,
    pub result: Option<VerificationResult>,
    pub status: VerificationStatus,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct VerificationResult {
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: String,
    pub constructor_arguments: Option<String>,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: String,
    pub sources: BTreeMap<String, String>,
}

impl From<(CompilerInput, CompilerVersion, VerificationSuccess)> for VerificationResult {
    fn from(
        (compiler_input, compiler_version, verification_success): (
            CompilerInput,
            CompilerVersion,
            VerificationSuccess,
        ),
    ) -> Self {
        VerificationResult {
            contract_name: verification_success.contract_name,
            compiler_version: compiler_version.to_string(),
            evm_version: compiler_input
                .settings
                .evm_version
                .map(|v| v.to_string())
                .unwrap_or_else(|| "default".to_string()),
            constructor_arguments: verification_success
                .constructor_args
                .map(|b| DisplayBytes::from(b).to_string()),
            optimization: compiler_input.settings.optimizer.enabled,
            optimization_runs: compiler_input.settings.optimizer.runs,
            contract_libraries: compiler_input
                .settings
                .libraries
                .libs
                .into_iter()
                .flat_map(|(_path, libs)| libs)
                .collect(),
            abi: serde_json::to_string(&verification_success.abi)
                .expect("Is result of local compilation and, thus, should be always valid"),
            sources: compiler_input
                .sources
                .into_iter()
                .map(|(path, source)| (path.to_string_lossy().to_string(), source.content))
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub enum VerificationStatus {
    #[serde(rename = "0")]
    Ok,
    #[serde(rename = "1")]
    Failed,
}

impl VerificationResponse {
    pub fn ok(result: VerificationResult) -> Self {
        Self {
            message: "OK".to_string(),
            result: Some(result),
            status: VerificationStatus::Ok,
        }
    }

    pub fn err(message: impl Display) -> Self {
        Self {
            message: message.to_string(),
            result: None,
            status: VerificationStatus::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_serialize_json_ok;
    use serde_json::json;

    #[test]
    fn parse_response() {
        test_serialize_json_ok(vec![
            (
                VerificationResponse::ok(VerificationResult {
                    contract_name: "contract_name".to_string(),
                    compiler_version: "compiler_version".to_string(),
                    evm_version: "evm_version".to_string(),
                    constructor_arguments: Some("constructor_arguments".to_string()),
                    optimization: Some(false),
                    optimization_runs: Some(200),
                    contract_libraries: BTreeMap::from([(
                        "some_library".into(),
                        "some_address".into(),
                    )]),
                    abi: "abi".to_string(),
                    sources: serde_json::from_str(
                        r#"{
                            "source.sol": "content"
                        }"#,
                    )
                    .unwrap(),
                }),
                json!({
                    "message": "OK",
                    "status": "0",
                    "result": {
                        "contract_name": "contract_name",
                        "compiler_version": "compiler_version",
                        "evm_version": "evm_version",
                        "constructor_arguments": "constructor_arguments",
                        "contract_libraries": {
                            "some_library": "some_address",
                        },
                        "optimization": false,
                        "optimization_runs": 200,
                        "abi": "abi",
                        "sources": {
                            "source.sol": "content",
                        },
                    },

                }),
            ),
            (
                VerificationResponse::err("Parse error"),
                json!({
                    "message": "Parse error",
                    "status": "1",
                    "result": null,
                }),
            ),
        ])
    }
}
