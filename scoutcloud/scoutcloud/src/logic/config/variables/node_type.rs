use crate::logic::config::macros;
use serde::{de::IntoDeserializer, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Parity,
    Erigon,
    Geth,
    Besu,
    Ganache,
}

macros::single_env_var!(
    NodeType,
    String,
    backend,
    "NODE_TYPE",
    Some("geth".to_string()),
    {
        fn validate(v: String) -> Result<(), anyhow::Error> {
            Self::deserialize(v.clone().into_deserializer())
                .map_err(|_: serde_json::Error| anyhow::anyhow!("unknown node_type: '{}'", v))?;
            Ok(())
        }
    }
);
