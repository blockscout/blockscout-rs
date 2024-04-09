use crate::logic::{
    config::ConfigError, ConfigValidationContext, ParsedVariable, ParsedVariableKey, UserVariable,
};
use serde::{Deserialize, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::str::FromStr;

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

derive_display_from_serialize!(ChainType);
derive_fromstr_from_deserialize!(ChainType);

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
impl UserVariable for ChainType {
    type SourceType = String;

    fn new(v: String, _context: &ConfigValidationContext) -> Result<Self, ConfigError> {
        Self::from_str(&v)
            .map_err(|_| ConfigError::Validation(format!("unknown chain_type: '{}'", v)))
    }
    async fn build_config_vars(
        &self,
        _context: &ConfigValidationContext,
    ) -> Result<Vec<ParsedVariable>, ConfigError> {
        let mut vars = vec![(
            ParsedVariableKey::BackendEnv("CHAIN_TYPE".to_string()),
            serde_json::Value::String(self.to_string()),
        )];

        if let Some(image) = self.maybe_custom_image() {
            vars.push((
                ParsedVariableKey::ConfigPath("blockscout.image.repository".to_string()),
                serde_json::Value::String(image),
            ));
        };

        Ok(vars)
    }
}
