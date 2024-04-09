use crate::logic::{
    config::ConfigError, ConfigValidationContext, ParsedVariable, ParsedVariableKey, UserVariable,
};
use serde::{Deserialize, Serialize};
use serde_plain::derive_fromstr_from_deserialize;
use std::str::FromStr;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerSize {
    Small,
    Medium,
    Large,
}
derive_fromstr_from_deserialize!(ServerSize);

impl ServerSize {
    // TODO: add database connection to Context and fetch resources from `server_specs` table
    pub fn resources(&self) -> serde_json::Value {
        match self {
            Self::Small => serde_json::json!({
              "limits": {
                "memory": "4Gi",
                "cpu": "2"
              },
              "requests": {
                "memory": "2Gi",
                "cpu": "1"
              }
            }),
            Self::Medium => serde_json::json!({
              "limits": {
                "memory": "8Gi",
                "cpu": "4"
              },
              "requests": {
                "memory": "4Gi",
                "cpu": "2"
              }
            }),
            Self::Large => serde_json::json!({
              "limits": {
                "memory": "16Gi",
                "cpu": "8"
              },
              "requests": {
                "memory": "8Gi",
                "cpu": "4"
              }
            }),
        }
    }
}

#[async_trait::async_trait]
impl UserVariable for ServerSize {
    type SourceType = String;

    fn new(v: String, _config: &ConfigValidationContext) -> Result<Self, ConfigError> {
        Self::from_str(&v)
            .map_err(|_| ConfigError::Validation(format!("unknown server_size: '{}'", v)))
    }

    async fn build_config_vars(
        &self,
        _config: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, ConfigError> {
        Ok(vec![(
            ParsedVariableKey::ConfigPath("blockscout.resources".to_string()),
            self.resources(),
        )])
    }
}
