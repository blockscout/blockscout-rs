use crate::logic::config::{variables::macros, ParsedVariable, UserVariable};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeTypeEnum {
    Parity,
    Erigon,
    Geth,
    Besu,
    Ganache,
}

macros::single_string_env_var!(node_type, backend, "NODE_TYPE", None, {
    fn validate(v: String) -> Result<(), anyhow::Error> {
        serde_json::from_str::<NodeTypeEnum>(&v)
            .map_err(|e| anyhow::anyhow!("unknown node_type: '{}'", v))?;
        Ok(())
    }
});
