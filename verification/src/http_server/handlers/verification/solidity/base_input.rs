use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct VerificationBase {
    pub contract_name: String,
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,
    pub constructor_arguments: Option<String>,
}
