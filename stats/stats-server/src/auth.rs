use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tonic::Status;

pub struct AuthorizationProvider {
    keys: HashMap<String, ApiKey>,
}

pub const API_KEY_NAME: &str = "x-api-key";

impl AuthorizationProvider {
    pub fn new(keys: HashMap<String, ApiKey>) -> Self {
        Self { keys }
    }

    pub fn is_request_authorized<T>(&self, request: &tonic::Request<T>) -> bool {
        let Some(key) = request.metadata().get(API_KEY_NAME) else {
            return false;
        };
        let Ok(api_key) = key
            .to_str()
            .inspect_err(|e| tracing::warn!("could not read http header as ascii: {}", e))
        else {
            return false;
        };
        self.is_key_authorized(api_key)
    }

    pub fn is_key_authorized(&self, api_key: &str) -> bool {
        self.keys.values().any(|key| key.key.eq(api_key))
    }

    /// Unified error message
    pub fn unauthorized(&self) -> Status {
        Status::unauthenticated(format!(
            "Request not authorized: Invalid or missing API key in {API_KEY_NAME} header"
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiKey {
    pub key: String,
}

impl ApiKey {
    pub fn new(key: String) -> Self {
        Self { key }
    }

    pub fn from_str_infallible(key: &str) -> Self {
        Self {
            key: key.to_string(),
        }
    }
}
