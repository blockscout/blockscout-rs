use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataHash {
    /// Do not include bytecode hash.
    #[serde(rename = "none")]
    None,
    /// The default keccak256 hash.
    #[serde(rename = "keccak256")]
    Keccak256,
}
