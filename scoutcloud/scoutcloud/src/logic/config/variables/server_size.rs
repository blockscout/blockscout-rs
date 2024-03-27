use crate::logic::config::{ParsedVariable, ParsedVariableKey, UserVariable};
use serde::{de::IntoDeserializer, Deserialize, Serialize};

pub struct ServerSize;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerSizeEnum {
    Small,
    Medium,
    Large,
}

impl ServerSizeEnum {
    pub fn resources(&self) -> serde_json::Value {
        match self {
            ServerSizeEnum::Small => serde_json::json!({
              "limits": {
                "memory": "4Gi",
                "cpu": "2"
              },
              "requests": {
                "memory": "2Gi",
                "cpu": "1"
              }
            }),
            ServerSizeEnum::Medium => serde_json::json!({
              "limits": {
                "memory": "8Gi",
                "cpu": "4"
              },
              "requests": {
                "memory": "4Gi",
                "cpu": "2"
              }
            }),
            ServerSizeEnum::Large => serde_json::json!({
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
        let ty = ServerSizeEnum::deserialize(v.clone().into_deserializer())
            .map_err(|_: serde_json::Error| anyhow::anyhow!("unknown server_size: '{}'", v))?;
        Ok(vec![(
            ParsedVariableKey::ConfigPath("blockscout.resources".to_string()),
            ty.resources(),
        )])
    }
}
