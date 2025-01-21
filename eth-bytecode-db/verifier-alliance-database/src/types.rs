use sea_orm::prelude::{DateTimeWithTimeZone, Uuid};
use std::collections::BTreeMap;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, Match, RuntimeCodeArtifacts,
};
use verifier_alliance_entity_v1::contract_deployments;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractDeployment {
    pub id: Uuid,
    pub chain_id: u128,
    pub address: Vec<u8>,
    pub runtime_code: Vec<u8>,
    pub creation_code: Option<Vec<u8>>,
    pub model: contract_deployments::Model,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ContractCode {
    OnlyRuntimeCode {
        code: Vec<u8>,
    },
    CompleteCode {
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum InsertContractDeployment {
    Genesis {
        chain_id: u128,
        address: Vec<u8>,
        runtime_code: Vec<u8>,
    },
    Regular {
        chain_id: u128,
        address: Vec<u8>,
        transaction_hash: Vec<u8>,
        block_number: u128,
        transaction_index: u128,
        deployer: Vec<u8>,
        creation_code: Vec<u8>,
        runtime_code: Vec<u8>,
    },
}

impl InsertContractDeployment {
    pub fn chain_id(&self) -> u128 {
        match self {
            InsertContractDeployment::Genesis { chain_id, .. } => *chain_id,
            InsertContractDeployment::Regular { chain_id, .. } => *chain_id,
        }
    }

    pub fn address(&self) -> &[u8] {
        match self {
            InsertContractDeployment::Genesis { address, .. } => address,
            InsertContractDeployment::Regular { address, .. } => address,
        }
    }

    pub fn runtime_code(&self) -> &[u8] {
        match self {
            InsertContractDeployment::Genesis { runtime_code, .. } => runtime_code,
            InsertContractDeployment::Regular { runtime_code, .. } => runtime_code,
        }
    }

    pub fn creation_code(&self) -> Option<&[u8]> {
        match self {
            InsertContractDeployment::Genesis { .. } => None,
            InsertContractDeployment::Regular { creation_code, .. } => Some(creation_code),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RetrieveContractDeployment {
    pub(crate) chain_id: u128,
    pub(crate) address: Vec<u8>,
    pub(crate) transaction_hash: Option<Vec<u8>>,
    pub(crate) runtime_code: Option<Vec<u8>>,
}

impl RetrieveContractDeployment {
    pub fn regular(chain_id: u128, address: Vec<u8>, transaction_hash: Vec<u8>) -> Self {
        Self {
            chain_id,
            address,
            transaction_hash: Some(transaction_hash),
            runtime_code: None,
        }
    }

    pub fn genesis(chain_id: u128, address: Vec<u8>, runtime_code: Vec<u8>) -> Self {
        Self {
            chain_id,
            address,
            transaction_hash: None,
            runtime_code: Some(runtime_code),
        }
    }

    pub fn chain_id(&self) -> u128 {
        self.chain_id
    }

    pub fn address(&self) -> &[u8] {
        &self.address
    }
}

#[derive(Clone, Debug, strum::Display, strum::EnumString, PartialEq, Eq, Hash)]
#[strum(serialize_all = "lowercase")]
pub enum CompiledContractCompiler {
    Solc,
    Vyper,
}

#[derive(Clone, Debug, strum::Display, strum::EnumString, PartialEq, Eq, Hash)]
#[strum(serialize_all = "lowercase")]
pub enum CompiledContractLanguage {
    Solidity,
    Yul,
    Vyper,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledContract {
    pub compiler: CompiledContractCompiler,
    pub version: String,
    pub language: CompiledContractLanguage,
    pub name: String,
    pub fully_qualified_name: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: serde_json::Value,
    pub compilation_artifacts: CompilationArtifacts,
    pub creation_code: Vec<u8>,
    pub creation_code_artifacts: CreationCodeArtifacts,
    pub runtime_code: Vec<u8>,
    pub runtime_code_artifacts: RuntimeCodeArtifacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VerifiedContractMatches {
    OnlyCreation {
        creation_match: Match,
    },
    OnlyRuntime {
        runtime_match: Match,
    },
    Complete {
        creation_match: Match,
        runtime_match: Match,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedContract {
    pub contract_deployment_id: Uuid,
    pub compiled_contract: CompiledContract,
    pub matches: VerifiedContractMatches,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrievedVerifiedContract {
    pub verified_contract: VerifiedContract,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub created_by: String,
    pub updated_by: String,
}
