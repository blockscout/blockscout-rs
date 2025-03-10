use super::{
    cbor_auxdata, compiler_output,
    evm_compilers::{CompilerInput, EvmCompiler, EvmCompilersPool},
    verification::RecompiledCode,
    Error,
};
use crate::{
    verify_new::compiler_output::SharedCompilerOutput, DetailedVersion, FullyQualifiedName,
    Language, Version,
};
use anyhow::Context;
use blockscout_display_bytes::decode_hex;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use verification_common::verifier_alliance::{
    CompilationArtifacts, CreationCodeArtifacts, LinkReferences, RuntimeCodeArtifacts,
};

pub type PerContractArtifacts = BTreeMap<FullyQualifiedName, CompiledContractArtifacts>;

#[derive(Clone, Debug, PartialEq)]
pub struct CompiledContractArtifacts {
    pub code: RecompiledCode,
    pub compilation_artifacts: CompilationArtifacts,
    pub creation_code_artifacts: CreationCodeArtifacts,
    pub runtime_code_artifacts: RuntimeCodeArtifacts,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilationResult {
    pub language: Language,
    pub compiler_version: String,
    pub compiler_settings: Value,
    pub sources: BTreeMap<String, String>,
    pub artifacts: PerContractArtifacts,
}

pub async fn compile<C: EvmCompiler>(
    compilers: &EvmCompilersPool<C>,
    compiler_version: &DetailedVersion,
    mut compiler_input: C::CompilerInput,
) -> Result<CompilationResult, Error> {
    let compiler_version = compilers.normalize_compiler_version(compiler_version)?;
    let compiler_path = compilers.fetch_compiler(&compiler_version).await?;

    compiler_input.normalize_output_selection(compiler_version.to_semver());
    let compiler_output = compilers
        .compile(&compiler_path, &compiler_version, &compiler_input)
        .await?;

    let modified_compiler_input = compiler_input.modified_copy();
    let modified_compiler_output = compilers
        .compile(&compiler_path, &compiler_version, &modified_compiler_input)
        .await?;

    let mut per_contract_artifacts = generate_per_contract_artifacts(compiler_output.output)?;
    let modified_per_contract_artifacts =
        generate_per_contract_artifacts(modified_compiler_output.output)?;

    let language = compiler_input.language();
    append_cbor_auxdata(
        language,
        &mut per_contract_artifacts,
        &modified_per_contract_artifacts,
    )?;

    Ok(CompilationResult {
        language,
        compiler_version: compiler_version.to_string(),
        compiler_settings: compiler_input.settings(),
        sources: compiler_input.sources(),
        artifacts: per_contract_artifacts,
    })
}

fn generate_per_contract_artifacts(
    compiler_output: SharedCompilerOutput,
) -> Result<PerContractArtifacts, Error> {
    let source_ids = extract_encoded_source_ids(&compiler_output.sources)?;

    let mut all_artifacts = BTreeMap::new();
    for (file_path, contracts) in compiler_output.contracts {
        for (contract_name, contract) in contracts {
            let fully_qualified_name =
                FullyQualifiedName::from_file_and_contract_names(file_path.clone(), contract_name);
            let contract_artifacts = generate_contract_artifacts(source_ids.clone(), contract)?;

            all_artifacts.insert(fully_qualified_name, contract_artifacts);
        }
    }

    Ok(all_artifacts)
}

fn extract_encoded_source_ids(
    sources: &compiler_output::SourceFiles,
) -> Result<Value, anyhow::Error> {
    #[derive(Serialize)]
    struct SourceId {
        id: u32,
    }

    let mut source_ids = BTreeMap::new();
    for (path, source) in sources {
        source_ids.insert(path, SourceId { id: source.id });
    }

    serde_json::to_value(source_ids).context("encoding source id values")
}

fn generate_contract_artifacts(
    source_ids: Value,
    contract: compiler_output::Contract,
) -> Result<CompiledContractArtifacts, anyhow::Error> {
    let runtime_code = extract_code_from_bytecode(&contract.evm.deployed_bytecode.bytecode)
        .context("extracting runtime code")?;
    let creation_code =
        extract_code_from_bytecode(&contract.evm.bytecode).context("extracting creation code")?;

    let artifacts = CompiledContractArtifacts {
        code: RecompiledCode {
            runtime: runtime_code,
            creation: creation_code,
        },
        compilation_artifacts: extract_compilation_artifacts(source_ids, &contract),
        creation_code_artifacts: extract_creation_code_artifacts(&contract),
        runtime_code_artifacts: extract_runtime_code_artifacts(&contract),
    };
    Ok(artifacts)
}

fn extract_compilation_artifacts(
    source_ids: Value,
    contract: &compiler_output::Contract,
) -> CompilationArtifacts {
    CompilationArtifacts {
        abi: contract.abi.clone(),
        devdoc: contract.devdoc.clone(),
        userdoc: contract.userdoc.clone(),
        storage_layout: contract.storage_layout.clone(),
        sources: Some(source_ids),
    }
}

fn extract_creation_code_artifacts(contract: &compiler_output::Contract) -> CreationCodeArtifacts {
    CreationCodeArtifacts {
        source_map: contract.evm.bytecode.source_map.clone(),
        link_references: contract.evm.bytecode.link_references.clone(),
        cbor_auxdata: None,
    }
}

fn extract_runtime_code_artifacts(contract: &compiler_output::Contract) -> RuntimeCodeArtifacts {
    RuntimeCodeArtifacts {
        source_map: contract.evm.deployed_bytecode.bytecode.source_map.clone(),
        immutable_references: contract.evm.deployed_bytecode.immutable_references.clone(),
        link_references: contract
            .evm
            .deployed_bytecode
            .bytecode
            .link_references
            .clone(),
        cbor_auxdata: None,
    }
}

fn extract_code_from_bytecode(
    bytecode: &compiler_output::Bytecode,
) -> Result<Vec<u8>, anyhow::Error> {
    let code = match &bytecode.object {
        compiler_output::BytecodeObject::Bytecode(bytes) => bytes.to_vec(),
        compiler_output::BytecodeObject::Unlinked(unlinked) => {
            let nullified = nullify_libraries(unlinked.clone(), &bytecode.link_references)
                .context("nullify unlinked libraries")?;
            decode_hex(&nullified).context("cannot decode resultant code as bytes")?
        }
    };
    Ok(code)
}

fn nullify_libraries(
    mut to_nullify: String,
    link_references: &Option<LinkReferences>,
) -> Result<String, anyhow::Error> {
    if let Some(link_references) = link_references.as_ref() {
        let offsets = link_references
            .values()
            .flat_map(|file_link_references| file_link_references.values())
            .flatten();
        for offset in offsets {
            // Offset stores start and length values for bytes, while code is a hex encoded string
            let start = offset.start as usize * 2;
            let length = offset.length as usize * 2;
            if to_nullify.len() < start + length {
                Err(anyhow::anyhow!("link reference offset exceeds code size"))?
            }

            to_nullify.replace_range(start..start + length, &"0".repeat(length));
        }
    }

    Ok(to_nullify)
}

fn append_cbor_auxdata(
    language: Language,
    artifacts: &mut PerContractArtifacts,
    modified_artifacts: &PerContractArtifacts,
) -> Result<(), Error> {
    for (fully_qualified_name, contract_artifacts) in artifacts.iter_mut() {
        let modified_contract_artifacts = modified_artifacts
            .get(fully_qualified_name)
            .expect("both artifacts and modified artifacts were compiled with the same contracts");

        contract_artifacts.creation_code_artifacts.cbor_auxdata =
            cbor_auxdata::retrieve_cbor_auxdata(
                language,
                &contract_artifacts.code.creation,
                &modified_contract_artifacts.code.creation,
            )?;
        contract_artifacts.runtime_code_artifacts.cbor_auxdata =
            cbor_auxdata::retrieve_cbor_auxdata(
                language,
                &contract_artifacts.code.runtime,
                &modified_contract_artifacts.code.runtime,
            )?;
    }

    Ok(())
}
