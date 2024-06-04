use serde::{Deserialize, Deserializer};
use std::collections::BTreeMap;
use std::ops::Deref;

#[derive(Clone, Debug)]
pub struct Lossless<T> {
    pub value: T,
    pub raw: serde_json::Value,
}

impl<'de, T: for<'tde> Deserialize<'tde>> Deserialize<'de> for Lossless<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let raw: serde_json::Value = Deserialize::deserialize(deserializer)?;
        let value: T = serde_json::from_value(raw.clone()).map_err(serde::de::Error::custom)?;
        Ok(Self { value, raw })
    }
}

impl<T: PartialEq> PartialEq for Lossless<T> {
    fn eq(&self, other: &Self) -> bool {
        self.eq(other)
    }
}

impl<T> Deref for Lossless<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

pub type LosslessCompilerOutput = Lossless<CompilerOutput>;

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct CompilerOutput {
    #[serde(default)]
    pub contracts: BTreeMap<String, BTreeMap<String, Contract>>,
    #[serde(default)]
    pub sources: SourceFiles,
}

pub type SourceFiles = BTreeMap<String, Lossless<SourceFile>>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct SourceFile {
    pub id: u32,
    pub ast: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    /// The Ethereum Contract ABI.
    /// See https://docs.soliditylang.org/en/develop/abi-spec.html
    pub abi: Option<serde_json::Value>,
    pub userdoc: Option<serde_json::Value>,
    pub devdoc: Option<serde_json::Value>,
    pub storage_layout: Option<serde_json::Value>,
    /// EVM-related outputs
    pub evm: Evm,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Evm {
    pub bytecode: Bytecode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Bytecode {
    #[serde(
        deserialize_with = "deserialize_bytes"
    )]
    pub object: bytes::Bytes,
}

pub fn deserialize_bytes<'de, D>(d: D) -> Result<bytes::Bytes, D::Error>
    where
        D: Deserializer<'de>,
{
    let value = String::deserialize(d)?;
    if let Some(value) = value.strip_prefix("0x") {
        hex::decode(value)
    } else {
        hex::decode(&value)
    }
        .map(Into::into)
        .map_err(|e| serde::de::Error::custom(e.to_string()))
}