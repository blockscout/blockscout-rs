use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// This struct is used as input for our endpoint and as
// input for sourcify endpoint at the same time
#[derive(Debug, Deserialize, Serialize, PartialEq)]
pub struct SourcifyRequest {
    pub address: String,
    pub chain: String,
    pub files: HashMap<String, String>,
}

// Definition of sourcify.dev API response
// https://docs.sourcify.dev/docs/api/server/v1/verify/
#[derive(Deserialize)]
#[serde(untagged)]
pub enum SourcifyVerifyResponse {
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

#[allow(unused)]
#[derive(Deserialize)]
pub struct ResultItem {
    address: String,
    status: String,
}

#[allow(unused)]
#[derive(Deserialize, Debug)]
pub struct FieldError {
    field: String,
    message: String,
}
