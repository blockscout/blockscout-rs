use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::Deserialize;
use std::{collections::BTreeMap, str::FromStr, sync::Arc};

#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub deployed_creation_code: Option<DisplayBytes>,
    pub deployed_runtime_code: DisplayBytes,

    pub compiled_creation_code: DisplayBytes,
    pub compiled_runtime_code: DisplayBytes,
    pub compiler: String,
    pub version: String,
    pub language: String,
    pub name: String,
    pub fully_qualified_name: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: serde_json::Value,
    pub compilation_artifacts: serde_json::Value,
    pub creation_code_artifacts: serde_json::Value,
    pub runtime_code_artifacts: serde_json::Value,

    pub creation_match: bool,
    pub creation_values: Option<serde_json::Value>,
    pub creation_transformations: Option<serde_json::Value>,

    pub runtime_match: bool,
    pub runtime_values: Option<serde_json::Value>,
    pub runtime_transformations: Option<serde_json::Value>,

    #[serde(default = "default_chain_id")]
    pub chain_id: usize,
    #[serde(default = "default_address")]
    pub address: DisplayBytes,
    #[serde(default = "default_transaction_hash")]
    pub transaction_hash: DisplayBytes,
    #[serde(default = "default_block_number")]
    pub block_number: i64,
    #[serde(default = "default_transaction_index")]
    pub transaction_index: i64,
    #[serde(default = "default_deployer")]
    pub deployer: DisplayBytes,

    #[serde(default)]
    pub is_genesis: bool,
}

impl TestCase {
    pub fn standard_input(&self) -> serde_json::Value {
        let input = foundry_compilers::CompilerInput {
            language: self.language.clone(),
            sources: self
                .sources
                .iter()
                .map(|(file_path, content)| {
                    (
                        std::path::PathBuf::from(file_path),
                        foundry_compilers::artifacts::Source {
                            content: Arc::new(content.clone()),
                        },
                    )
                })
                .collect(),
            settings: serde_json::from_value(self.compiler_settings.clone())
                .expect("settings deserialization"),
        };

        serde_json::to_value(&input).unwrap()
    }

    pub fn contract_name(&self) -> String {
        self.fully_qualified_name
            .split(':')
            .last()
            .unwrap()
            .to_string()
    }

    pub fn file_name(&self) -> String {
        let name_parts: Vec<_> = self.fully_qualified_name.split(':').collect();
        name_parts
            .into_iter()
            .rev()
            .skip(1)
            .rev()
            .collect::<Vec<_>>()
            .join(":")
    }
}

fn default_chain_id() -> usize {
    5
}
fn default_address() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafe").unwrap()
}
fn default_transaction_hash() -> DisplayBytes {
    DisplayBytes::from_str("0xcafecafecafecafecafecafecafecafecafecafecafecafecafecafecafecafe")
        .unwrap()
}
fn default_block_number() -> i64 {
    1
}
fn default_transaction_index() -> i64 {
    0
}
fn default_deployer() -> DisplayBytes {
    DisplayBytes::from_str("0xfacefacefacefacefacefacefacefacefaceface").unwrap()
}
