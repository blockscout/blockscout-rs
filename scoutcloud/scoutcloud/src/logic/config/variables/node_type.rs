use crate::logic::{
    config::{macros, Error},
    ConfigValidationContext,
};
use serde::{Deserialize, Serialize};
use serde_plain::{derive_display_from_serialize, derive_fromstr_from_deserialize};
use std::str::FromStr;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Parity,
    Erigon,
    Geth,
    Besu,
    Ganache,
}
derive_display_from_serialize!(NodeType);
derive_fromstr_from_deserialize!(NodeType);

macros::custom_env_var!(NodeType, String, BackendEnv, "NODE_TYPE", {
    fn new(v: String, _config: &ConfigValidationContext) -> Result<Self, Error> {
        Self::from_str(&v).map_err(|_| Error::Validation(format!("unknown node_type: '{}'", v)))
    }

    fn maybe_default(_context: &ConfigValidationContext) -> Option<String> {
        Some("ganache".to_string())
    }
});
