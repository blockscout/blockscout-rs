use crate::logic::{ParsedVariable, ParsedVariableKey, UserVariable};
use serde::{de::IntoDeserializer, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainType {
    Ethereum,
    PolygonEdge,
    PolygonZkevm,
    Rsk,
    Shibarium,
    Stability,
    Suave,
    Zetachain,
    Filecoin,
    Default,
}

impl ChainType {
    pub fn maybe_custom_image(&self) -> Option<String> {
        let image = match self {
            Self::PolygonEdge => Some("blockscout/blockscout-polygon-edge"),
            Self::PolygonZkevm => Some("blockscout/blockscout-zkevm"),
            Self::Rsk => Some("blockscout/blockscout-rsk"),
            Self::Shibarium => Some("blockscout/blockscout-shibarium"),
            Self::Stability => Some("blockscout/blockscout-stability"),
            Self::Suave => Some("blockscout/blockscout-suave"),
            Self::Zetachain => None,
            Self::Filecoin => None,
            Self::Default => None,
            Self::Ethereum => None,
        };

        image.map(str::to_string)
    }
}

#[async_trait::async_trait]
impl UserVariable<String> for ChainType {
    async fn build_config_vars(v: String) -> Result<Vec<ParsedVariable>, anyhow::Error> {
        let ty = Self::deserialize(v.clone().into_deserializer())
            .map_err(|_: serde_json::Error| anyhow::anyhow!("unknown chain_type: '{}'", v))?;
        let mut vars = vec![(
            ParsedVariableKey::BackendEnv("CHAIN_TYPE".to_string()),
            serde_json::json!(&ty),
        )];

        if let Some(image) = ty.maybe_custom_image() {
            vars.push((
                ParsedVariableKey::ConfigPath("blockscout.image.repository".to_string()),
                serde_json::json!(image),
            ));
        };

        Ok(vars)
    }
}
