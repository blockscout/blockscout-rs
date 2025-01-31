use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tonic::Status;

pub struct AuthorizationProvider {
    keys: HashMap<String, ApiKey>,
}

const API_KEY_NAME: &str = "x-api-key";

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
        self.keys.values().find(|key| key.key.eq(api_key)).is_some()
    }

    /// Unified error message
    pub fn unauthorized(&self) -> Status {
        Status::unauthenticated("invalid api key")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiKey {
    pub key: String,
}
