use crate::logic::config::{variables::macros, ParsedVariable, UserVariable};
use serde::{de::IntoDeserializer, Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeTypeEnum {
    Parity,
    Erigon,
    Geth,
    Besu,
    Ganache,
}

macros::single_string_env_var!(node_type, backend, "NODE_TYPE", Some("geth".to_string()), {
    fn validate(v: String) -> Result<(), anyhow::Error> {
        NodeTypeEnum::deserialize(v.clone().into_deserializer())
            .map_err(|_: serde_json::Error| anyhow::anyhow!("unknown node_type: '{}'", v))?;
        Ok(())
    }
});
