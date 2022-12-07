use entity::{sea_orm_active_enums::PartType, sources};
use sea_orm::DatabaseConnection;
use serde::Deserialize;
use std::collections::BTreeMap;

use super::insert_verification::insert_verification_result;

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct VerificationResult {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub evm_version: String,
    pub constructor_arguments: Option<String>,
    pub optimization: Option<bool>,
    pub optimization_runs: Option<usize>,
    pub contract_libraries: BTreeMap<String, String>,
    pub abi: Option<String>,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: String,
    pub local_creation_input_parts: Vec<BytecodePart>,
    pub local_deployed_bytecode_parts: Vec<BytecodePart>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PartTy {
    Main,
    Meta,
}

impl From<PartTy> for PartType {
    fn from(ty: PartTy) -> Self {
        match ty {
            PartTy::Main => PartType::Main,
            PartTy::Meta => PartType::Metadata,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct BytecodePart {
    pub data: String,
    pub r#type: PartTy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContractType {
    Small,
    Medium,
    Big,
    Constructor,
}

pub async fn generate_and_insert(
    db: &DatabaseConnection,
    id: usize,
    ty: ContractType,
) -> Result<sources::Model, anyhow::Error> {
    let verification_result = VerificationResult::generate(id, ty);
    insert_verification_result(db, verification_result).await
}

impl VerificationResult {
    pub fn generate(id: usize, ty: ContractType) -> Self {
        match ty {
            ContractType::Small => {
                let template = include_str!("contracts/type_1.json");
                Self::from_template(template, id).expect("should be valid verification result")
            }
            ContractType::Medium => {
                let template = include_str!("contracts/type_2.json");
                Self::from_template(template, id).expect("should be valid verification result")
            }
            ContractType::Big => {
                let template = include_str!("contracts/type_3.json");
                Self::from_template(template, id).expect("should be valid verification result")
            }
            ContractType::Constructor => {
                let template = include_str!("contracts/type_4.json");
                Self::from_template(template, id).expect("should be valid verification result")
            }
        }
    }

    fn from_template(template: &str, id: usize) -> Result<Self, serde_json::Error> {
        serde_json::from_str(&template.replace("{{ID}}", &format!("{:0>10}", id)))
    }
}
