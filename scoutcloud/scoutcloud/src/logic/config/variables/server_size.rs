use crate::logic::{ParsedVariable, ParsedVariableKey, UserVariable};
use serde::{de::IntoDeserializer, Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerSize {
    Small,
    Medium,
    Large,
}

impl ServerSize {
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
impl UserVariable<String> for ServerSize {
    async fn build_config_vars(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        let ty = Self::deserialize(v.clone().into_deserializer())
            .map_err(|_: serde_json::Error| anyhow::anyhow!("unknown server_size: '{}'", v))?;
        Ok(vec![(
            ParsedVariableKey::ConfigPath("blockscout.resources".to_string()),
            ty.resources(),
        )])
    }
}
