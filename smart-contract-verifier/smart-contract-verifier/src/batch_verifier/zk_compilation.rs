use std::collections::BTreeMap;
use bytes::Bytes;
use crate::batch_verifier::{zk_artifacts};
use anyhow::Context;
use super::compilation;
use crate::batch_verifier::compilation::CompilationResult;
use super::zk_lossless_output;

#[derive(Clone, Debug)]
pub struct ParsedContract {
    pub file_name: String,
    pub contract_name: String,
    pub creation_code: Bytes,
    pub compilation_artifacts: zk_artifacts::compilation_artifacts::CompilationArtifacts,
    pub creation_code_artifacts: zk_artifacts::creation_code_artifacts::CreationCodeArtifacts,
}

#[derive(Clone, Debug)]
pub struct ZkCompilationResult {
    pub zk_compiler: String,
    pub zk_compiler_version: String,
    pub compiler_version: String,
    pub language: String,
    pub compiler_settings: serde_json::Value,
    pub sources: BTreeMap<String, String>,
    pub parsed_contracts: Vec<ParsedContract>,
}

pub use zksolc::parse_contracts as parse_zksolc_contracts;
mod zksolc {
    use serde::Deserialize;
    use super::*;
    use crate::batch_verifier::{decode_hex, zk_artifacts};
    use zk_artifacts::cbor_auxdata::{self, CborAuxdata};
    use crate::{CompactVersion, DetailedVersion};

    fn to_lossless_output<T: for <'de> Deserialize<'de>>(
        raw: serde_json::Value,
    ) -> Result<T, anyhow::Error> {
        serde_json::from_value(raw)
            .map_err(|err| anyhow::anyhow!("cannot parse compiler output in lossless format: {err}"))
    }

    pub fn parse_contracts(
        zk_compiler_version: CompactVersion,
        evm_compiler_version: DetailedVersion,
        compiler_input: &crate::zksolc_standard_json::input::Input,
        compiler_output: serde_json::Value,
        modified_compiler_output: serde_json::Value,
    ) -> Result<ZkCompilationResult, anyhow::Error> {
        let compiler_output: zk_lossless_output::LosslessCompilerOutput = to_lossless_output(compiler_output).context("original output")?;
        let modified_compiler_output: zk_lossless_output::CompilerOutput =
            to_lossless_output(modified_compiler_output).context("modified output")?;

        let mut parsed_contracts: Vec<ParsedContract> = Vec::new();
        // Here we are re-using the fact that BTreeMaps::into_iter
        // produces items in order by key.
        for ((file_name, contracts), (modified_file_name, modified_contracts)) in compiler_output
            .contracts
            .iter()
            .zip(&modified_compiler_output.contracts)
        {
            if file_name != modified_file_name {
                anyhow::bail!(
                    "file={file_name} - modified file name does not correspond to original one: {modified_file_name}"
                )
            }

            for ((contract_name, contract), (modified_contract_name, modified_contract)) in
            contracts.iter().zip(modified_contracts)
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

        Ok(ZkCompilationResult {
            zk_compiler: "zksolc".to_string(),
            zk_compiler_version: zk_compiler_version.to_string(),
            compiler_version: evm_compiler_version.to_string(),
            language: compiler_input.language.to_string(),
            compiler_settings: serde_json::to_value(compiler_input.settings.clone())
                .expect("settings should serialize into valid json"),
            sources: compiler_input
                .sources
                .iter()
                .map(|(file, source)| {
                    (
                        file.clone(),
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
        source_files: &zk_lossless_output::SourceFiles,
        contract: &zk_lossless_output::Contract,
        modified_contract: &zk_lossless_output::Contract,
    ) -> Result<ParsedContract, anyhow::Error> {
        let (creation_code, creation_cbor_auxdata) =
            parse_code_details(&contract.evm.bytecode, &modified_contract.evm.bytecode)
                .context("parse creation code details")?;

        let compilation_artifacts =
            zk_artifacts::compilation_artifacts::generate(contract, source_files);
        let creation_code_artifacts =
            zk_artifacts::creation_code_artifacts::generate(contract, creation_cbor_auxdata);

        Ok(ParsedContract {
            file_name,
            contract_name,
            compilation_artifacts,
            creation_code,
            creation_code_artifacts,
        })
    }

    fn parse_code_details(
        code: &zk_lossless_output::Bytecode,
        modified_code: &zk_lossless_output::Bytecode,
    ) -> Result<(Bytes, CborAuxdata), anyhow::Error> {
        let code = preprocess_code(code).context("preprocess original output")?;
        let modified_code = preprocess_code(modified_code).context("preprocess modified output")?;

        let bytecode_parts =
            crate::verifier::split(&code, &modified_code).context("split on bytecode parts")?;
        let cbor_auxdata = cbor_auxdata::generate(&bytecode_parts);

        Ok((code, cbor_auxdata))
    }

    fn preprocess_code(
        code_bytecode: &zk_lossless_output::Bytecode,
    ) -> Result<Bytes, anyhow::Error> {
        Ok(code_bytecode.object.clone())
    }
}
