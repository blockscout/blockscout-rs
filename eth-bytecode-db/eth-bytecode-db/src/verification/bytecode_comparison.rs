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
        creation_match = verifier_alliance::verify_creation_code(
            &on_chain_creation_code,
            re_compiled_contract.creation_code.clone(),
            &re_compiled_contract.creation_code_artifacts,
            &re_compiled_contract.compilation_artifacts,
        )?;
    }

    let runtime_match = verifier_alliance::verify_runtime_code(
        &contract_deployment.runtime_code,
        re_compiled_contract.runtime_code.clone(),
        &re_compiled_contract.runtime_code_artifacts,
    )?;

    let verified_contract_matches = match (creation_match, runtime_match) {
        (Some(creation_match), Some(runtime_match)) => VerifiedContractMatches::Complete {
            creation_match,
            runtime_match,
        },
        (Some(creation_match), None) => VerifiedContractMatches::OnlyCreation { creation_match },
        (None, Some(runtime_match)) => VerifiedContractMatches::OnlyRuntime { runtime_match },
        (None, None) => {
            return Err(anyhow::anyhow!(
                "neither creation code nor runtime code did not match"
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
        creation_code: &str,
        runtime_code: &str,
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
        let on_chain_creation_code = "0x608060405234801561001057600080fd5b50610133806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";
        let on_chain_runtime_code = "0x6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";

        let request = VerifySolidityStandardJsonRequest {
            bytecode: on_chain_creation_code.into(),
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
            contract_deployment(address, on_chain_creation_code, on_chain_runtime_code);

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

    #[tokio::test]
    async fn should_verify_partial_match() {
        let address = "0x1052623FD0425d216A7c2FB466f6DbF216323282";
        let on_chain_creation_code = "0x608060405234801561001057600080fd5b50610133806100206000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";
        let on_chain_runtime_code = "0x6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea26469706673582212204ac0ce5f82b26331fa3e9ae959291a55624ffaf90fcd509deafcc21a5f1da21e64736f6c63430008120033";
        let request = VerifySolidityStandardJsonRequest {
            bytecode: on_chain_creation_code.into(),
            bytecode_type: BytecodeType::CreationInput as i32,
            compiler_version: "v0.8.18+commit.87f61d96".into(),
            input: "{\"language\":\"Solidity\",\"sources\":{\"contracts/1_Storage.sol\":{\"content\":\"// SPDX-License-Identifier: GPL-3.0\\n\\npragma solidity >=0.7.0 <0.9.0;\\n\\n/**\\n * @title Storage\\n * @dev Store & retrieve value in a variable\\n */\\ncontract Storage {\\n    uint256 public number;\\n\\n    /**\\n     * @dev Store value in variable\\n     * @param modified_num value to store\\n     */\\n    function store(uint256 modified_num) public {\\n        number = modified_num;\\n    }\\n}\"}},\"settings\":{\"optimizer\":{\"enabled\":false,\"runs\":200},\"libraries\":{},\"outputSelection\":{\"*\":{\"*\":[\"*\"]}}}}".into(),
            metadata: Some(VerificationMetadata {
                chain_id: Some("11155111".into()),
                contract_address: Some(address.into()),
            }),
            post_actions: vec![],
        };
        let source = verify_contract(request).await;

        let contract_deployment =
            contract_deployment(address, on_chain_creation_code, on_chain_runtime_code);

        let verified_contract = check_code_matches_new(&source, contract_deployment)
            .expect("check code matches failed");
        match verified_contract.matches {
            VerifiedContractMatches::Complete {
                creation_match,
                runtime_match,
            } => {
                assert!(
                    !creation_match.metadata_match,
                    "creation metadata should not match"
                );
                assert_eq!(
                    creation_match.transformations.len(),
                    1,
                    "invalid creation transformations length"
                );
                assert!(
                    !runtime_match.metadata_match,
                    "runtime metadata should not match"
                );
                assert_eq!(
                    runtime_match.transformations.len(),
                    1,
                    "invalid runtime transformations length"
                )
            }
            value => panic!("not complete match: {value:#?}"),
        }
    }

    #[tokio::test]
    async fn should_verify_partial_match_with_double_auxdata() {
        let address = "0x383eEaD9Ceeb888827d40D9f194882B36590378a";
        let on_chain_creation_code = "0x60806040526040518060200161001490610049565b6020820181038252601f19601f820116604052506001908161003691906102a5565b5034801561004357600080fd5b50610377565b605c8061069c83390190565b600081519050919050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806100d657607f821691505b6020821081036100e9576100e861008f565b5b50919050565b60008190508160005260206000209050919050565b60006020601f8301049050919050565b600082821b905092915050565b6000600883026101517fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff82610114565b61015b8683610114565b95508019841693508086168417925050509392505050565b6000819050919050565b6000819050919050565b60006101a261019d61019884610173565b61017d565b610173565b9050919050565b6000819050919050565b6101bc83610187565b6101d06101c8826101a9565b848454610121565b825550505050565b600090565b6101e56101d8565b6101f08184846101b3565b505050565b5b81811015610214576102096000826101dd565b6001810190506101f6565b5050565b601f8211156102595761022a816100ef565b61023384610104565b81016020851015610242578190505b61025661024e85610104565b8301826101f5565b50505b505050565b600082821c905092915050565b600061027c6000198460080261025e565b1980831691505092915050565b6000610295838361026b565b9150826002028217905092915050565b6102ae82610055565b67ffffffffffffffff8111156102c7576102c6610060565b5b6102d182546100be565b6102dc828285610218565b600060209050601f83116001811461030f57600084156102fd578287015190505b6103078582610289565b86555061036f565b601f19841661031d866100ef565b60005b8281101561034557848901518255600182019150602085019450602081019050610320565b86831015610362578489015161035e601f89168261026b565b8355505b6001600288020188555050505b505050505050565b610316806103866000396000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c806324c12bf6146100465780636057361d146100645780638381f58a14610080575b600080fd5b61004e61009e565b60405161005b91906101cc565b60405180910390f35b61007e60048036038101906100799190610229565b61012c565b005b610088610136565b6040516100959190610265565b60405180910390f35b600180546100ab906102af565b80601f01602080910402602001604051908101604052809291908181526020018280546100d7906102af565b80156101245780601f106100f957610100808354040283529160200191610124565b820191906000526020600020905b81548152906001019060200180831161010757829003601f168201915b505050505081565b8060008190555050565b60005481565b600081519050919050565b600082825260208201905092915050565b60005b8381101561017657808201518184015260208101905061015b565b60008484015250505050565b6000601f19601f8301169050919050565b600061019e8261013c565b6101a88185610147565b93506101b8818560208601610158565b6101c181610182565b840191505092915050565b600060208201905081810360008301526101e68184610193565b905092915050565b600080fd5b6000819050919050565b610206816101f3565b811461021157600080fd5b50565b600081359050610223816101fd565b92915050565b60006020828403121561023f5761023e6101ee565b5b600061024d84828501610214565b91505092915050565b61025f816101f3565b82525050565b600060208201905061027a6000830184610256565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806102c757607f821691505b6020821081036102da576102d9610280565b5b5091905056fea2646970667358221220bc2c6d72c52842d4077bb24c307576e44a078831aaa16da6611ef342fd052ec764736f6c634300081200336080604052348015600f57600080fd5b50603f80601d6000396000f3fe6080604052600080fdfea2646970667358221220f13d144a826a3f18798a534a4b10029a3284d9f4620ccc79750cdc48442cdaad64736f6c63430008120033";
        let on_chain_runtime_code = "0x608060405234801561001057600080fd5b50600436106100415760003560e01c806324c12bf6146100465780636057361d146100645780638381f58a14610080575b600080fd5b61004e61009e565b60405161005b91906101cc565b60405180910390f35b61007e60048036038101906100799190610229565b61012c565b005b610088610136565b6040516100959190610265565b60405180910390f35b600180546100ab906102af565b80601f01602080910402602001604051908101604052809291908181526020018280546100d7906102af565b80156101245780601f106100f957610100808354040283529160200191610124565b820191906000526020600020905b81548152906001019060200180831161010757829003601f168201915b505050505081565b8060008190555050565b60005481565b600081519050919050565b600082825260208201905092915050565b60005b8381101561017657808201518184015260208101905061015b565b60008484015250505050565b6000601f19601f8301169050919050565b600061019e8261013c565b6101a88185610147565b93506101b8818560208601610158565b6101c181610182565b840191505092915050565b600060208201905081810360008301526101e68184610193565b905092915050565b600080fd5b6000819050919050565b610206816101f3565b811461021157600080fd5b50565b600081359050610223816101fd565b92915050565b60006020828403121561023f5761023e6101ee565b5b600061024d84828501610214565b91505092915050565b61025f816101f3565b82525050565b600060208201905061027a6000830184610256565b92915050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052602260045260246000fd5b600060028204905060018216806102c757607f821691505b6020821081036102da576102d9610280565b5b5091905056fea2646970667358221220bc2c6d72c52842d4077bb24c307576e44a078831aaa16da6611ef342fd052ec764736f6c63430008120033";
        let request = VerifySolidityStandardJsonRequest {
            bytecode: on_chain_creation_code.into(),
            bytecode_type: BytecodeType::CreationInput as i32,
            compiler_version: "v0.8.18+commit.87f61d96".into(),
            input: "{\"language\":\"Solidity\",\"sources\":{\"contracts/1_Storage.sol\":{\"content\":\"// SPDX-License-Identifier: GPL-3.0\\n\\npragma solidity >=0.7.0 <0.9.0;\\n\\ncontract A {}\\n\\n/**\\n * @title Storage\\n * @dev Store & retrieve value in a variable\\n */\\ncontract Storage {\\n    uint256 public number;\\n\\n    // Comment to update metadata hash\\n    bytes public code = type(A).creationCode;\\n\\n    /**\\n     * @dev Store value in variable\\n     * @param num value to store\\n     */\\n    function store(uint256 num) public {\\n        number = num;\\n    }\\n}\"}},\"settings\":{\"optimizer\":{\"enabled\":false,\"runs\":200},\"libraries\":{},\"outputSelection\":{\"*\":{\"*\":[\"*\"]}}}}".into(),
            metadata: Some(VerificationMetadata {
                chain_id: Some("11155111".into()),
                contract_address: Some(address.into()),
            }),
            post_actions: vec![],
        };
        let source = verify_contract(request).await;

        let contract_deployment =
            contract_deployment(address, on_chain_creation_code, on_chain_runtime_code);

        let verified_contract = check_code_matches_new(&source, contract_deployment)
            .expect("check code matches failed");
        match verified_contract.matches {
            VerifiedContractMatches::Complete {
                creation_match,
                runtime_match,
            } => {
                assert!(
                    !creation_match.metadata_match,
                    "creation metadata should not match"
                );
                assert_eq!(
                    creation_match.transformations.len(),
                    2,
                    "invalid creation transformations length"
                );
                assert!(
                    !runtime_match.metadata_match,
                    "runtime metadata should not match"
                );
                assert_eq!(
                    runtime_match.transformations.len(),
                    1,
                    "invalid runtime transformations length"
                )
            }
            value => panic!("not complete match: {value:#?}"),
        }
    }

    #[tokio::test]
    async fn should_verify_immutables() {
        let address = "0xe2c3685fD385077504A389A0Fd569Fab7E54dB7d";
        let on_chain_creation_code = "0x60a0604052606460809081525034801561001857600080fd5b5060805161019a610033600039600060b0015261019a6000f3fe608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a146100625780639fe44c4a14610080575b600080fd5b610060600480360381019061005b919061010d565b61009e565b005b61006a6100a8565b6040516100779190610149565b60405180910390f35b6100886100ae565b6040516100959190610149565b60405180910390f35b8060008190555050565b60005481565b7f000000000000000000000000000000000000000000000000000000000000000081565b600080fd5b6000819050919050565b6100ea816100d7565b81146100f557600080fd5b50565b600081359050610107816100e1565b92915050565b600060208284031215610123576101226100d2565b5b6000610131848285016100f8565b91505092915050565b610143816100d7565b82525050565b600060208201905061015e600083018461013a565b9291505056fea26469706673582212205fff17b2676425e48225435ac15579ccae1af038ff8ffb334fc372526b94722664736f6c63430008120033";
        let on_chain_runtime_code = "0x608060405234801561001057600080fd5b50600436106100415760003560e01c80636057361d146100465780638381f58a146100625780639fe44c4a14610080575b600080fd5b610060600480360381019061005b919061010d565b61009e565b005b61006a6100a8565b6040516100779190610149565b60405180910390f35b6100886100ae565b6040516100959190610149565b60405180910390f35b8060008190555050565b60005481565b7f000000000000000000000000000000000000000000000000000000000000006481565b600080fd5b6000819050919050565b6100ea816100d7565b81146100f557600080fd5b50565b600081359050610107816100e1565b92915050565b600060208284031215610123576101226100d2565b5b6000610131848285016100f8565b91505092915050565b610143816100d7565b82525050565b600060208201905061015e600083018461013a565b9291505056fea26469706673582212205fff17b2676425e48225435ac15579ccae1af038ff8ffb334fc372526b94722664736f6c63430008120033";
        let request = VerifySolidityStandardJsonRequest {
            bytecode: on_chain_creation_code.into(),
            bytecode_type: BytecodeType::CreationInput as i32,
            compiler_version: "v0.8.18+commit.87f61d96".into(),
            input: "{\"language\":\"Solidity\",\"sources\":{\"contracts/1_Storage.sol\":{\"content\":\"// SPDX-License-Identifier: GPL-3.0\\n\\npragma solidity >=0.7.0 <0.9.0;\\n\\n/**\\n * @title Storage\\n * @dev Store & retrieve value in a variable\\n */\\ncontract Storage {\\n    uint256 public number;\\n\\n    uint256 public immutable imm_number = 100;\\n\\n    /**\\n     * @dev Store value in variable\\n     * @param num value to store\\n     */\\n    function store(uint256 num) public {\\n        number = num;\\n    }\\n}\"}},\"settings\":{\"optimizer\":{\"enabled\":false,\"runs\":200},\"libraries\":{},\"outputSelection\":{\"*\":{\"*\":[\"*\"]}}}}".into(),
            metadata: Some(VerificationMetadata {
                chain_id: Some("11155111".into()),
                contract_address: Some(address.into()),
            }),
            post_actions: vec![],
        };
        let source = verify_contract(request).await;

        let contract_deployment =
            contract_deployment(address, on_chain_creation_code, on_chain_runtime_code);

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
                assert_eq!(
                    creation_match.transformations.len(),
                    0,
                    "invalid creation transformations length"
                );
                assert!(
                    runtime_match.metadata_match,
                    "runtime metadata should match"
                );
                assert_eq!(
                    runtime_match.transformations.len(),
                    1,
                    "invalid runtime transformations length"
                )
            }
            value => panic!("not complete match: {value:#?}"),
        }
    }

    #[tokio::test]
    async fn should_verify_constructor_arguments() {
        let address = "0x664EEA330e41684EFE308014A4Ba358Bc079a853";
        let on_chain_creation_code = "0x608060405234801561001057600080fd5b506040516101e93803806101e98339818101604052810190610032919061007a565b80600081905550506100a7565b600080fd5b6000819050919050565b61005781610044565b811461006257600080fd5b50565b6000815190506100748161004e565b92915050565b6000602082840312156100905761008f61003f565b5b600061009e84828501610065565b91505092915050565b610133806100b66000396000f3fe6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea2646970667358221220dd712ec4cb31d63cd32d3152e52e890b087769e9e4d6746844608039b5015d6a64736f6c634300081200330000000000000000000000000000000000000000000000000000000000003039";
        let on_chain_runtime_code = "0x6080604052348015600f57600080fd5b506004361060325760003560e01c80636057361d1460375780638381f58a14604f575b600080fd5b604d60048036038101906049919060af565b6069565b005b60556073565b6040516060919060e4565b60405180910390f35b8060008190555050565b60005481565b600080fd5b6000819050919050565b608f81607e565b8114609957600080fd5b50565b60008135905060a9816088565b92915050565b60006020828403121560c25760c16079565b5b600060ce84828501609c565b91505092915050565b60de81607e565b82525050565b600060208201905060f7600083018460d7565b9291505056fea2646970667358221220dd712ec4cb31d63cd32d3152e52e890b087769e9e4d6746844608039b5015d6a64736f6c63430008120033";
        let request = VerifySolidityStandardJsonRequest {
            bytecode: on_chain_creation_code.into(),
            bytecode_type: BytecodeType::CreationInput as i32,
            compiler_version: "v0.8.18+commit.87f61d96".into(),
            input: "{\"language\":\"Solidity\",\"sources\":{\"contracts/1_Storage.sol\":{\"content\":\"// SPDX-License-Identifier: GPL-3.0\\n\\npragma solidity >=0.7.0 <0.9.0;\\n\\n/**\\n * @title Storage\\n * @dev Store & retrieve value in a variable\\n */\\ncontract Storage {\\n    uint256 public number;\\n\\n    constructor(uint256 num) {\\n        number = num;\\n    }\\n\\n    /**\\n     * @dev Store value in variable\\n     * @param num value to store\\n     */\\n    function store(uint256 num) public {\\n        number = num;\\n    }\\n}\"}},\"settings\":{\"optimizer\":{\"enabled\":false,\"runs\":200},\"libraries\":{},\"outputSelection\":{\"*\":{\"*\":[\"*\"]}}}}".into(),
            metadata: Some(VerificationMetadata {
                chain_id: Some("11155111".into()),
                contract_address: Some(address.into()),
            }),
            post_actions: vec![],
        };
        let source = verify_contract(request).await;

        let contract_deployment =
            contract_deployment(address, on_chain_creation_code, on_chain_runtime_code);

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
                assert_eq!(
                    creation_match.transformations.len(),
                    1,
                    "invalid creation transformations length"
                );
                assert!(
                    runtime_match.metadata_match,
                    "runtime metadata should match"
                );
                assert_eq!(
                    runtime_match.transformations.len(),
                    0,
                    "invalid runtime transformations length"
                )
            }
            value => panic!("not complete match: {value:#?}"),
        }
    }
}
