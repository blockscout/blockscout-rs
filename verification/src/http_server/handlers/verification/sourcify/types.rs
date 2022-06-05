use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// This struct is used as input for our endpoint and as
// input for sourcify endpoint at the same time
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct ApiRequest {
    pub address: String,
    pub chain: String,
    pub files: HashMap<String, String>,
}

// Definition of sourcify.dev API response
// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ApiVerificationResponse {
    Verified {
        #[allow(unused)]
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

#[allow(unused)]
#[derive(Deserialize)]
pub(super) struct ResultItem {
    address: String,
    status: String,
}

#[allow(unused)]
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
