use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

// This struct is used as input for our endpoint and as
// input for sourcify endpoint at the same time
#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApiRequest {
    pub address: String,
    pub chain: String,
    pub files: Files,
    pub chosen_contract: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Files(pub BTreeMap<String, String>);

#[derive(Debug, PartialEq, Eq)]
pub struct Success {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: String,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub constructor_arguments: Option<Bytes>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: String,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("{0:#}")]
    Internal(anyhow::Error),
    #[error("verification error: {0}")]
    Verification(String),
    #[error("validation error: {0}")]
    Validation(String),
}

// Definition of sourcify.dev API response
// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum ApiVerificationResponse {
    Verified {
        result: Vec<ResultItem>,
    },
    Error {
        error: String,
    },
    ValidationErrors {
        message: String,
        errors: Vec<FieldError>,
    },
}

#[derive(Deserialize, Serialize)]
pub(super) struct ResultItem {
    pub address: String,
    pub status: String,
    #[serde(rename = "storageTimestamp")]
    pub storage_timestamp: Option<String>,
}

#[derive(Deserialize, Debug, Serialize)]
pub(super) struct FieldError {
    field: String,
    message: String,
}

#[derive(Deserialize, Debug)]
pub(super) struct ApiFilesResponse {
    pub files: Vec<FileItem>,
}

#[derive(Deserialize, Debug)]
pub(super) struct FileItem {
    pub name: String,
    pub content: String,
}

impl TryFrom<ApiFilesResponse> for Files {
    type Error = anyhow::Error;

    fn try_from(response: ApiFilesResponse) -> Result<Self, Self::Error> {
        let files_map =
            BTreeMap::from_iter(response.files.into_iter().map(|f| (f.name, f.content)));
        Ok(Files(files_map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_deserialize_ok;
    use std::collections::BTreeMap;

    #[test]
    fn deserialize_api_request() {
        let inputs = Vec::from([
            (
                r#"{
                    "address": "0xcafecafecafecafecafecafecafecafecafecafe",
                    "chain": "100",
                    "files": {
                        "source.sol": "pragma ...",
                        "metadata.json": "{ metadata: ... }"
                    }
                }"#,
                ApiRequest {
                    address: "0xcafecafecafecafecafecafecafecafecafecafe".to_string(),
                    chain: "100".to_string(),
                    files: Files(BTreeMap::from([
                        ("source.sol".to_string(), "pragma ...".to_string()),
                        ("metadata.json".to_string(), "{ metadata: ... }".to_string()),
                    ])),
                    chosen_contract: None,
                },
            ),
            (
                r#"{
                    "address": "0xcafecafecafecafecafecafecafecafecafecafe",
                    "chain": "100",
                    "files": {
                        "source.sol": "pragma ...",
                        "metadata.json": "{ metadata: ... }"
                    },
                    "chosenContract": 1
                }"#,
                ApiRequest {
                    address: "0xcafecafecafecafecafecafecafecafecafecafe".to_string(),
                    chain: "100".to_string(),
                    files: Files(BTreeMap::from([
                        ("source.sol".to_string(), "pragma ...".to_string()),
                        ("metadata.json".to_string(), "{ metadata: ... }".to_string()),
                    ])),
                    chosen_contract: Some(1),
                },
            ),
        ]);

        test_deserialize_ok(inputs);
    }
}
