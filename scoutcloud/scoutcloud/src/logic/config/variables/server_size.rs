use crate::logic::config::{ParsedVariable, ParsedVariableKey, UserVariable};
use serde::Deserialize;

pub struct ServerSize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerSizeType {
    Small,
    Medium,
    Large,
}

impl ServerSizeType {
    pub fn resources(&self) -> serde_yaml::Value {
        let json = match self {
            ServerSizeType::Small => serde_json::json!({
              "limits": {
                "memory": "4Gi",
                "cpu": "2"
              },
              "requests": {
                "memory": "2Gi",
                "cpu": "1"
              }
            }),
            ServerSizeType::Medium => serde_json::json!({
              "limits": {
                "memory": "8Gi",
                "cpu": "4"
              },
              "requests": {
                "memory": "4Gi",
                "cpu": "2"
              }
            }),
            ServerSizeType::Large => serde_json::json!({
              "limits": {
                "memory": "16Gi",
                "cpu": "8"
              },
              "requests": {
                "memory": "8Gi",
                "cpu": "4"
              }
            }),
        };
        serde_json::from_value(json).expect("valid json2yaml object")
    }
}

#[async_trait::async_trait]
impl UserVariable<String> for ServerSize {
    async fn parse_from_value(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        let ty = serde_json::from_str::<ServerSizeType>(&v)
            .map_err(|e| anyhow::anyhow!("unknown server size: '{}'", v))?;
        Ok(vec![(
            ParsedVariableKey::ConfigPath(".".to_string()),
            ty.resources(),
        )])
    }
}
