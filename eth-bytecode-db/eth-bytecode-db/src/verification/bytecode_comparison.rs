#![allow(dead_code)]

use crate::verification::{BytecodePart, Source, SourceType};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use verification_common_v1::verifier_alliance;
use verifier_alliance_database::{
    CompiledContract, CompiledContractCompiler, CompiledContractLanguage, ContractDeployment,
    VerifiedContract, VerifiedContractMatches,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseReadySourceNew {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<serde_json::Value>,
    pub compilation_artifacts: Option<verifier_alliance::CompilationArtifacts>,
    pub creation_code_artifacts: Option<verifier_alliance::CreationCodeArtifacts>,
    pub runtime_code_artifacts: Option<verifier_alliance::RuntimeCodeArtifacts>,

    pub raw_creation_code: Vec<u8>,
    pub raw_runtime_code: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl TryFrom<Source> for DatabaseReadySourceNew {
    type Error = anyhow::Error;

    fn try_from(value: Source) -> Result<Self, Self::Error> {
        let abi = value
            .abi
            .map(|abi| serde_json::from_str(&abi).context("deserialize abi into json value"))
            .transpose()?;
        let compiler_settings: serde_json::Value =
            serde_json::from_str(&value.compiler_settings)
                .context("deserialize compiler settings into json value")?;
        let compilation_artifacts: Option<verifier_alliance::CompilationArtifacts> = value
            .compilation_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize compilation artifacts into json value")?;
        let creation_input_artifacts: Option<verifier_alliance::CreationCodeArtifacts> = value
            .creation_input_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize creation input artifacts into json value")?;
        let deployed_bytecode_artifacts: Option<verifier_alliance::RuntimeCodeArtifacts> = value
            .deployed_bytecode_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize deployed bytecode artifacts into json value")?;

        Ok(Self {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings,
            source_type: value.source_type,
            source_files: value.source_files,
            abi,
            compilation_artifacts,
            creation_code_artifacts: creation_input_artifacts,
            runtime_code_artifacts: deployed_bytecode_artifacts,
            raw_creation_code: value.raw_creation_input,
            raw_runtime_code: value.raw_deployed_bytecode,
            creation_input_parts: value.creation_input_parts,
            deployed_bytecode_parts: value.deployed_bytecode_parts,
        })
    }
}

fn check_code_matches_new(
    database_source: &DatabaseReadySourceNew,
    contract_deployment: ContractDeployment,
) -> Result<VerifiedContract, anyhow::Error> {
    let re_compiled_contract = build_compiled_contract(database_source.clone())?;

    let mut creation_match = None;
    if let Some(on_chain_creation_code) = contract_deployment.creation_code {
        creation_match = verifier_alliance::MatchBuilder::new(
            &on_chain_creation_code,
            re_compiled_contract.creation_code.clone(),
        )
        .map(|builder| {
            builder.apply_creation_code_transformations(
                &re_compiled_contract.creation_code_artifacts,
                &re_compiled_contract.compilation_artifacts,
            )
        })
        .transpose()?
        .and_then(|builder| builder.verify_and_build());
    }

    let runtime_match = verifier_alliance::MatchBuilder::new(
        &contract_deployment.runtime_code,
        re_compiled_contract.runtime_code.clone(),
    )
    .map(|builder| {
        builder.apply_runtime_code_transformations(&re_compiled_contract.runtime_code_artifacts)
    })
    .transpose()?
    .and_then(|builder| builder.verify_and_build());

    let verified_contract_matches = match (creation_match, runtime_match) {
        (Some(creation_match), Some(runtime_match)) => VerifiedContractMatches::Complete {
            creation_match,
            runtime_match,
        },
        (Some(creation_match), None) => VerifiedContractMatches::OnlyCreation { creation_match },
        (None, Some(runtime_match)) => VerifiedContractMatches::OnlyRuntime { runtime_match },
        (None, None) => {
            return Err(anyhow::anyhow!(
                "Neither creation code nor runtime code have not matched"
            ))
        }
    };

    Ok(VerifiedContract {
        contract_deployment_id: contract_deployment.id,
        compiled_contract: re_compiled_contract,
        matches: verified_contract_matches,
    })
}

fn build_compiled_contract(
    source: DatabaseReadySourceNew,
) -> Result<CompiledContract, anyhow::Error> {
    let compilation_artifacts = source
        .compilation_artifacts
        .ok_or(anyhow::anyhow!("compilation artifacts are missing"))?;
    let creation_code_artifacts = source
        .creation_code_artifacts
        .ok_or(anyhow::anyhow!("creation code artifacts are missing"))?;
    let runtime_code_artifacts = source
        .runtime_code_artifacts
        .ok_or(anyhow::anyhow!("runtime code artifacts are missing"))?;

    let (compiler, language) = match source.source_type {
        SourceType::Solidity => (
            CompiledContractCompiler::Solc,
            CompiledContractLanguage::Solidity,
        ),
        SourceType::Vyper => (
            CompiledContractCompiler::Vyper,
            CompiledContractLanguage::Vyper,
        ),
        SourceType::Yul => (
            CompiledContractCompiler::Solc,
            CompiledContractLanguage::Yul,
        ),
    };

    let fully_qualified_name = format!("{}:{}", source.file_name, source.contract_name);

    let compiled_contract = CompiledContract {
        compiler,
        version: source.compiler_version,
        language,
        name: source.contract_name,
        fully_qualified_name,
        sources: source.source_files,
        compiler_settings: source.compiler_settings,
        compilation_artifacts,
        creation_code: source.raw_creation_code,
        creation_code_artifacts,
        runtime_code: source.raw_runtime_code,
        runtime_code_artifacts,
    };
    Ok(compiled_contract)
}

#[cfg(test)]
mod tests {
    use super::*;
    use smart_contract_verifier::v2::{
        BytecodeType, VerificationMetadata, VerifySolidityStandardJsonRequest,
    };
    use smart_contract_verifier_proto::{
        blockscout::smart_contract_verifier,
        http_client::{
            solidity_verifier_client, Client as VerifierClient, Config as VerifierConfig,
        },
    };

    const VERIFIER_URL: &str = "https://http.sc-verifier-test.k8s-dev.blockscout.com";

    async fn verify_contract(request: VerifySolidityStandardJsonRequest) -> DatabaseReadySourceNew {
        let verifier_client = VerifierClient::new(VerifierConfig::new(VERIFIER_URL.into())).await;
        let response = solidity_verifier_client::verify_standard_json(&verifier_client, request)
            .await
            .expect("error verifying contract");

        let source = crate::verification::handlers::from_response_to_source(response)
            .await
            .expect("error parsing response into source");
        DatabaseReadySourceNew::try_from(source)
            .expect("error converting source to database ready source")
    }

    fn contract_deployment(
        address: &str,
        runtime_code: &str,
        creation_code: &str,
    ) -> ContractDeployment {
        let address = blockscout_display_bytes::decode_hex(address).unwrap();
        let runtime_code = blockscout_display_bytes::decode_hex(runtime_code).unwrap();
        let creation_code = blockscout_display_bytes::decode_hex(creation_code).unwrap();
        ContractDeployment {
            id: Default::default(),
            chain_id: 11155111,
            address,
            runtime_code,
            creation_code: Some(creation_code),
            model: verifier_alliance_entity_v1::contract_deployments::Model {
                id: Default::default(),
                created_at: Default::default(),
                updated_at: Default::default(),
                created_by: "".to_string(),
                updated_by: "".to_string(),
                chain_id: Default::default(),
                address: vec![],
                transaction_hash: vec![],
                block_number: Default::default(),
                transaction_index: Default::default(),
                deployer: vec![],
                contract_id: Default::default(),
            },
        }
    }

    #[tokio::test]
    async fn should_verify_full_match() {
        let address = "0xa851c68517290a357ec974D0a00A2f832322DdbA";
        let re_compiled_creation_code = "0x608060405234801561001057600080fd5b50610133806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";
        let re_compiled_runtime_code = "0x6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";
        let request = VerifySolidityStandardJsonRequest {
            bytecode: "0x608060405234801561001057600080fd5b50610133806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033".into(),
            bytecode_type: BytecodeType::CreationInput as i32,
            compiler_version: "v0.8.18+commit.87f61d96".into(),
            input: "{\"language\":  \"Solidity\", \"sources\": {\"contracts/1_Storage.sol\":  {\"content\":  \"// SPDX-License-Identifier: GPL-3.0\\n\\npragma solidity >=0.7.0 <0.9.0;\\n\\n/**\\n * @title Storage\\n * @dev Store & retrieve value in a variable\\n */\\ncontract Storage {\\n    uint256 public number;\\n\\n    /**\\n     * @dev Store value in variable\\n     * @param num value to store\\n     */\\n    function store(uint256 num) public {\\n        number = num;\\n    }\\n}\"}}, \"settings\":  {\"optimizer\":{\"enabled\":false,\"runs\":200},\"libraries\":{},\"outputSelection\":{\"*\":{\"*\":[\"*\"]}}}}".into(),
            metadata: Some(VerificationMetadata {
                chain_id: Some("11155111".into()),
                contract_address: Some(address.into()),
            }),
            post_actions: vec![],
        };
        let source = verify_contract(request).await;
        let contract_deployment =
            contract_deployment(address, re_compiled_runtime_code, re_compiled_creation_code);

        let verified_contract = check_code_matches_new(&source, contract_deployment)
            .expect("check code matches failed");
        match verified_contract.matches {
            VerifiedContractMatches::Complete {
                creation_match,
                runtime_match,
            } => {
                assert!(
                    creation_match.metadata_match,
                    "creation metadata should match"
                );
                assert!(
                    creation_match.transformations.is_empty(),
                    "creation transformations should be empty"
                );
                assert!(
                    runtime_match.metadata_match,
                    "runtime metadata should match"
                );
                assert!(
                    runtime_match.transformations.is_empty(),
                    "runtime transformations should be empty"
                )
            }
            value => panic!("not complete match: {value:#?}"),
        }
    }
}
