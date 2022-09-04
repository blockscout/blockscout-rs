use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

// Definition of sourcify.dev API response
// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize)]
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

#[derive(Deserialize)]
pub(super) struct ResultItem {
    pub address: String,
    pub status: String,
    #[serde(rename = "storageTimestamp")]
    pub storage_timestamp: Option<String>,
}

#[derive(Deserialize, Debug)]
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
    use super::{ApiRequest, Files};
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
