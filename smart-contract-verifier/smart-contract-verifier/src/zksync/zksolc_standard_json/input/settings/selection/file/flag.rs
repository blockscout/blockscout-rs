//!
//! The `solc --standard-json` expected output selection flag.
//!

use serde::{Deserialize, Serialize};

///
/// The `solc --standard-json` expected output selection flag.
///
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Flag {
    /// The ABI JSON.
    #[serde(rename = "abi")]
    ABI,
    /// The metadata.
    #[serde(rename = "metadata")]
    Metadata,
    /// The developer documentation.
    #[serde(rename = "devdoc")]
    Devdoc,
    /// The user documentation.
    #[serde(rename = "userdoc")]
    Userdoc,
    /// The function signature hashes JSON.
    #[serde(rename = "evm.methodIdentifiers")]
    MethodIdentifiers,
    /// The storage layout.
    #[serde(rename = "storageLayout")]
    StorageLayout,
    /// The AST JSON.
    #[serde(rename = "ast")]
    AST,
    /// The Yul IR.
    #[serde(rename = "irOptimized")]
    Yul,
    /// The EVM legacy assembly JSON.
    #[serde(rename = "evm.legacyAssembly")]
    EVMLA,
}

impl std::fmt::Display for Flag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ABI => write!(f, "abi"),
            Self::Metadata => write!(f, "metadata"),
            Self::Devdoc => write!(f, "devdoc"),
            Self::Userdoc => write!(f, "userdoc"),
            Self::MethodIdentifiers => write!(f, "evm.methodIdentifiers"),
            Self::StorageLayout => write!(f, "storageLayout"),
            Self::AST => write!(f, "ast"),
            Self::Yul => write!(f, "irOptimized"),
            Self::EVMLA => write!(f, "evm.legacyAssembly"),
        }
    }
}
