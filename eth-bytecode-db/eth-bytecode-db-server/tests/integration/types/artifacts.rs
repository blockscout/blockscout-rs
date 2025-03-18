use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fmt::{Debug, Formatter},
};

macro_rules! impl_lossless {
    ($name:ident) => {
        paste::paste! {
            #[derive(Clone, Debug, PartialEq)]
            pub struct [<Lossless $name>] {
                pub parsed: $name,
                pub raw: serde_json::Value,
            }

            impl std::fmt::Display for [<Lossless $name>] {
                fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Display::fmt(&self.raw, f)
                }
            }

            impl<'de> Deserialize<'de> for [<Lossless $name>] {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                    where
                        D: serde::Deserializer<'de>,
                {
                    let raw: serde_json::Value = Deserialize::deserialize(deserializer)?;

                    let parsed: $name = serde_json::from_value(raw.clone()).map_err(serde::de::Error::custom)?;

                    Ok(Self { parsed, raw })
                }
            }
        }
    };
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilationArtifacts {
    pub abi: Option<serde_json::Value>,
    pub devdoc: Option<serde_json::Value>,
    pub userdoc: Option<serde_json::Value>,
    pub storage_layout: Option<serde_json::Value>,
    pub sources: Option<serde_json::Value>,
}
impl_lossless!(CompilationArtifacts);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationCodeArtifacts {
    pub link_references: Option<serde_json::Value>,
    pub source_map: Option<String>,
    pub cbor_auxdata: Option<serde_json::Value>,
}
impl_lossless!(CreationCodeArtifacts);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCodeArtifacts {
    pub immutable_references: Option<serde_json::Value>,
    pub link_references: Option<serde_json::Value>,
    pub source_map: Option<String>,
    pub cbor_auxdata: Option<serde_json::Value>,
}
impl_lossless!(RuntimeCodeArtifacts);

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodeValues {
    pub cbor_auxdata: Option<BTreeMap<String, String>>,
    pub constructor_arguments: Option<String>,
    pub immutables: Option<BTreeMap<String, String>>,
    pub libraries: Option<BTreeMap<String, String>>,
}
impl_lossless!(CodeValues);

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CodeParts {
    pub r#type: String,
    pub data: blockscout_display_bytes::Bytes,
}
impl_lossless!(CodeParts);

type CompilerSettings = foundry_compilers::artifacts::Settings;
impl_lossless!(CompilerSettings);

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_lossless() {
        let raw = serde_json::json!({
            "abi": [{"hello": "world"}],
            "sources": {
                "Hi.sol": "AD",
                "WO.sol": "World"
            },
            "unknown": "heey"
        });
        let lossless: LosslessCompilationArtifacts = serde_json::from_value(raw.clone()).unwrap();
        let expected_parsed: CompilationArtifacts = serde_json::from_value(raw.clone()).unwrap();

        assert_eq!(raw, lossless.raw, "Invalid raw value");
        assert_eq!(expected_parsed, lossless.parsed, "Invalid parsed value");
    }
}
