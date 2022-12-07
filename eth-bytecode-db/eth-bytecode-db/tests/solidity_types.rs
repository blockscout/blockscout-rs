use blockscout_display_bytes::Bytes as DisplayBytes;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestCase {
    #[serde(rename = "_comment")]
    pub comment: Option<String>,

    pub deployed_bytecode: DisplayBytes,
    pub creation_input: DisplayBytes,
    pub compiler_version: String,

    pub source_files: Option<BTreeMap<String, String>>,

    pub contract_name: String,
    pub source_code: String,
    pub expected_constructor_argument: Option<DisplayBytes>,

    pub is_deployed_bytecode: bool,
}
