use super::artifacts::{self};
use crate::{verifier::lossless_compiler_output, DetailedVersion};
use anyhow::Context;
use artifacts::LinkReferences;
use bytes::Bytes;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct ParsedContract {
    pub file_name: String,
    pub contract_name: String,
    pub creation_code: Bytes,
    pub runtime_code: Bytes,
    pub compilation_artifacts: artifacts::compilation_artifacts::CompilationArtifacts,
    pub creation_code_artifacts: artifacts::creation_code_artifacts::CreationCodeArtifacts,
    pub runtime_code_artifacts: artifacts::runtime_code_artifacts::RuntimeCodeArtifacts,
}

#[derive(Clone, Debug)]
pub struct CompilationResult {
    pub compiler: String,
    pub compiler_version: String,
    pub language: String,
    pub compiler_settings: serde_json::Value,
    pub sources: BTreeMap<String, String>,
    pub parsed_contracts: Vec<ParsedContract>,
}

fn to_lossless_output(
    raw: serde_json::Value,
) -> Result<lossless_compiler_output::CompilerOutput, anyhow::Error> {
    serde_json::from_value(raw)
        .map_err(|err| anyhow::anyhow!("cannot parse compiler output in lossless format: {err}"))
}

pub use solidity::parse_contracts as parse_solidity_contracts;
mod solidity {
    use super::*;
    use crate::batch_verifier::decode_hex;
    use artifacts::cbor_auxdata::{self, CborAuxdata};

    pub fn parse_contracts(
        compiler_version: DetailedVersion,
        compiler_input: &foundry_compilers::CompilerInput,
        compiler_output: serde_json::Value,
        modified_compiler_output: serde_json::Value,
    ) -> Result<CompilationResult, anyhow::Error> {
        let compiler_output = to_lossless_output(compiler_output).context("original output")?;
        let modified_compiler_output =
            to_lossless_output(modified_compiler_output).context("modified output")?;

        let mut parsed_contracts = Vec::new();
        // Here we are re-using the fact that BTreeMaps::into_iter
        // produces items in order by key.
        for ((file_name, contracts), (modified_file_name, modified_contracts)) in compiler_output
            .contracts
            .into_iter()
            .zip(modified_compiler_output.contracts)
        {
            if file_name != modified_file_name {
                anyhow::bail!(
                "file={file_name} - modified file name does not correspond to original one: {modified_file_name}"
            )
            }

            for ((contract_name, contract), (modified_contract_name, modified_contract)) in
                contracts.into_iter().zip(modified_contracts)
            {
                if contract_name != modified_contract_name {
                    anyhow::bail!(
                    "file={file_name}; contract={contract_name} - \
                    modified contract name does not correspond to original one: {modified_contract_name}"
                )
                }

                let parsed_contract = parse_contract(
                    file_name.clone(),
                    contract_name.clone(),
                    &compiler_output.sources.clone(),
                    &contract,
                    &modified_contract,
                )
                .context(format!(
                    "parsing contract; file={file_name}, contract={contract_name}"
                ))?;

                parsed_contracts.push(parsed_contract);
            }
        }

        Ok(CompilationResult {
            compiler: "solc".to_string(),
            compiler_version: compiler_version.to_string(),
            language: compiler_input.language.clone(),
            compiler_settings: serde_json::to_value(compiler_input.settings.clone())
                .expect("settings should serialize into valid json"),
            sources: compiler_input
                .sources
                .iter()
                .map(|(file, source)| {
                    (
                        file.to_string_lossy().to_string(),
                        source.content.to_string(),
                    )
                })
                .collect(),
            parsed_contracts,
        })
    }

    fn parse_contract(
        file_name: String,
        contract_name: String,
        source_files: &lossless_compiler_output::SourceFiles,
        contract: &lossless_compiler_output::Contract,
        modified_contract: &lossless_compiler_output::Contract,
    ) -> Result<ParsedContract, anyhow::Error> {
        let (creation_code, creation_cbor_auxdata) =
            parse_code_details(&contract.evm.bytecode, &modified_contract.evm.bytecode)
                .context("parse creation code details")?;
        let (runtime_code, runtime_cbor_auxdata) = parse_code_details(
            &contract.evm.deployed_bytecode.bytecode,
            &modified_contract.evm.deployed_bytecode.bytecode,
        )
        .context("parse runtime code details")?;

        let compilation_artifacts =
            artifacts::compilation_artifacts::generate(contract, source_files);
        let creation_code_artifacts =
            artifacts::creation_code_artifacts::generate(contract, creation_cbor_auxdata);
        let runtime_code_artifacts =
            artifacts::runtime_code_artifacts::generate(contract, runtime_cbor_auxdata);

        Ok(ParsedContract {
            file_name,
            contract_name,
            compilation_artifacts,
            creation_code,
            creation_code_artifacts,
            runtime_code,
            runtime_code_artifacts,
        })
    }

    fn parse_code_details(
        code: &lossless_compiler_output::Bytecode,
        modified_code: &lossless_compiler_output::Bytecode,
    ) -> Result<(Bytes, CborAuxdata), anyhow::Error> {
        let code = preprocess_code(code).context("preprocess original output")?;
        let modified_code = preprocess_code(modified_code).context("preprocess modified output")?;

        let bytecode_parts =
            crate::verifier::split(&code, &modified_code).context("split on bytecode parts")?;
        let cbor_auxdata = cbor_auxdata::generate(&bytecode_parts);

        Ok((code, cbor_auxdata))
    }

    fn preprocess_code(
        code_bytecode: &lossless_compiler_output::Bytecode,
    ) -> Result<Bytes, anyhow::Error> {
        let code_link_references = code_bytecode
            .link_references
            .as_ref()
            .map(|references| serde_json::from_value::<LinkReferences>(references.clone()))
            .transpose()
            .map_err(|err| anyhow::anyhow!("deserializing code link references failed: {err}"))?
            .unwrap_or_default();
        let code = match code_bytecode.object.clone() {
            foundry_compilers::artifacts::BytecodeObject::Bytecode(bytes) => bytes.0,
            foundry_compilers::artifacts::BytecodeObject::Unlinked(value) => {
                nullify_libraries(value, code_link_references).context("nullify libraries")?
            }
        };
        Ok(code)
    }

    fn nullify_libraries(
        mut code: String,
        link_references: LinkReferences,
    ) -> Result<Bytes, anyhow::Error> {
        let offsets = link_references
            .into_values()
            .flat_map(|file_link_references| file_link_references.into_values())
            .flatten();
        for offset in offsets {
            // Offset stores start and length values for bytes, while code is a hex encoded string
            let start = offset.start as usize * 2;
            let length = offset.length as usize * 2;
            if code.len() < start + length {
                anyhow::bail!("link reference offset exceeds code size")
            }

            code.replace_range(start..start + length, &"0".repeat(length));
        }

        let result = decode_hex(&code)
            .map_err(|err| anyhow::anyhow!("cannot format bytecode as bytes {err}"))?;

        Ok(Bytes::from(result))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use ethers_core::types::Bytes;
        use pretty_assertions::assert_eq;
        use std::str::FromStr;

        #[test]
        fn test_nullify_libraries() {
            let code = "608060405234801561000f575f80fd5b506101d78061001d5f395ff3fe608060405234801561000f575f80fd5b5060043610610029575f3560e01c80631003e2d21461002d575b5f80fd5b61004760048036038101906100429190610101565b610049565b005b73__$381a49d83ac7aa68573c6404d0bf9b6c49$__63cad0899b5f54836040518363ffffffff1660e01b815260040161008392919061013b565b602060405180830381865af415801561009e573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906100c29190610176565b5f8190555050565b5f80fd5b5f819050919050565b6100e0816100ce565b81146100ea575f80fd5b50565b5f813590506100fb816100d7565b92915050565b5f60208284031215610116576101156100ca565b5b5f610123848285016100ed565b91505092915050565b610135816100ce565b82525050565b5f60408201905061014e5f83018561012c565b61015b602083018461012c565b9392505050565b5f81519050610170816100d7565b92915050565b5f6020828403121561018b5761018a6100ca565b5b5f61019884828501610162565b9150509291505056fea26469706673582212209b4b28e8ef54b8fa1f251c01babde84cbe2a44a99d5bffe3cab53ee14c9addd164736f6c63430008180033";
            let link_references = BTreeMap::from([(
                "contracts/Libs.sol".to_string(),
                BTreeMap::from([(
                    "Sum".to_string(),
                    vec![foundry_compilers::artifacts::Offsets {
                        start: 104,
                        length: 20,
                    }],
                )]),
            )]);

            let expected = Bytes::from_str("608060405234801561000f575f80fd5b506101d78061001d5f395ff3fe608060405234801561000f575f80fd5b5060043610610029575f3560e01c80631003e2d21461002d575b5f80fd5b61004760048036038101906100429190610101565b610049565b005b73000000000000000000000000000000000000000063cad0899b5f54836040518363ffffffff1660e01b815260040161008392919061013b565b602060405180830381865af415801561009e573d5f803e3d5ffd5b505050506040513d601f19601f820116820180604052508101906100c29190610176565b5f8190555050565b5f80fd5b5f819050919050565b6100e0816100ce565b81146100ea575f80fd5b50565b5f813590506100fb816100d7565b92915050565b5f60208284031215610116576101156100ca565b5b5f610123848285016100ed565b91505092915050565b610135816100ce565b82525050565b5f60408201905061014e5f83018561012c565b61015b602083018461012c565b9392505050565b5f81519050610170816100d7565b92915050565b5f6020828403121561018b5761018a6100ca565b5b5f61019884828501610162565b9150509291505056fea26469706673582212209b4b28e8ef54b8fa1f251c01babde84cbe2a44a99d5bffe3cab53ee14c9addd164736f6c63430008180033").unwrap();
            let actual =
                nullify_libraries(code.to_string(), link_references).expect("should succeed");
            assert_eq!(expected, actual)
        }
    }
}
