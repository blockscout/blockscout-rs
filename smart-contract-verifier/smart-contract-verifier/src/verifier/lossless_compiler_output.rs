use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompilerOutput {
    #[serde(default)]
    pub contracts: BTreeMap<String, BTreeMap<String, Contract>>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    /// The Ethereum Contract Metadata.
    /// See <https://docs.soliditylang.org/en/develop/metadata.html>
    pub abi: Option<serde_json::Value>,
    #[serde(default)]
    pub userdoc: Option<serde_json::Value>,
    #[serde(default)]
    pub devdoc: Option<serde_json::Value>,
    #[serde(default)]
    pub storage_layout: Option<serde_json::Value>,
    /// EVM-related outputs
    #[serde(default)]
    pub evm: Option<Evm>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evm {
    pub bytecode: Option<Bytecode>,
    #[serde(default)]
    pub deployed_bytecode: Option<DeployedBytecode>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bytecode {
    /// The source mapping as a string. See the source mapping definition.
    #[serde(default)]
    pub source_map: Option<String>,
    /// If given, this is an unlinked object.
    #[serde(default)]
    pub link_references: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeployedBytecode {
    /// The source mapping as a string. See the source mapping definition.
    #[serde(default)]
    pub source_map: Option<String>,
    /// If given, this is an unlinked object.
    #[serde(default)]
    pub link_references: Option<serde_json::Value>,
    #[serde(default)]
    pub immutable_references: Option<serde_json::Value>,
}
