use crate::{
    batch_verifier::{
        artifacts::cbor_auxdata::{self, CborAuxdata},
        compilation::{CompilationResult, ParsedSolidityContract},
        decode_hex,
        errors::{VerificationError, VerificationErrorKind},
        transformations,
    },
    compiler,
    verifier::{lossless_compiler_output, CompilerInput},
    BytecodePart, Compilers, Contract, SolidityCompiler, Version,
};
use anyhow::{anyhow, Context};
use bytes::{Buf, Bytes};
use ethers_solc::artifacts::Offsets;
use mismatch::Mismatch;
use solidity_metadata::MetadataHash;
use std::collections::BTreeMap;
use thiserror::Error;

type LinkReferences = BTreeMap<String, BTreeMap<String, Vec<Offsets>>>;

#[derive(Error, Debug)]
pub enum BatchError {
    #[error("Compiler version not found: {0}")]
    VersionNotFound(Version),
    #[error("Compilation error: {0:?}")]
    Compilation(Vec<String>),
    #[error("{0}")]
    Internal(anyhow::Error),
}

impl From<compiler::Error> for BatchError {
    fn from(error: compiler::Error) -> Self {
        match error {
            compiler::Error::VersionNotFound(version) => BatchError::VersionNotFound(version),
            compiler::Error::Compilation(details) => BatchError::Compilation(details),
            err => BatchError::Internal(anyhow!(err)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Match {
    pub values: serde_json::Value,
    pub transformations: serde_json::Value,
}

#[derive(Clone, Debug)]
pub enum VerificationResult {
    Success(BatchSuccess),
    Failure(Vec<VerificationError>),
}

#[derive(Clone, Debug, Default)]
pub struct BatchSuccess {
    pub compiler: String,
    pub compiler_version: String,
    pub language: String,
    pub compiler_settings: serde_json::Value,
    pub creation_code: Vec<u8>,
    pub runtime_code: Vec<u8>,
    pub file_name: String,
    pub contract_name: String,
    pub sources: BTreeMap<String, String>,
    pub compilation_artifacts: serde_json::Value,
    pub creation_code_artifacts: serde_json::Value,
    pub runtime_code_artifacts: serde_json::Value,
    pub creation_match: Option<Match>,
    pub runtime_match: Option<Match>,
}

pub async fn verify_solidity(
    compilers: &Compilers<SolidityCompiler>,
    compiler_version: Version,
    contracts: Vec<Contract>,
    compiler_input: &foundry_compilers::CompilerInput,
) -> Result<Vec<VerificationResult>, BatchError> {
    let (raw_compiler_output, _) = compilers
        .compile(&compiler_version, compiler_input, None)
        .await?;

    let (modified_raw_compiler_output, _) = {
        let compiler_input = compiler_input.clone().modify();
        compilers
            .compile(&compiler_version, &compiler_input, None)
            .await?
    };

    let compilation_result = parse_contracts(
        compiler_version,
        compiler_input,
        raw_compiler_output,
        modified_raw_compiler_output,
    )?;

    let mut results = vec![];
    for contract in contracts {
        results.push(verify_contract(contract, &compilation_result)?);
    }

    Ok(results)
}

fn verify_contract(
    contract: Contract,
    compilation_result: &CompilationResult,
) -> Result<VerificationResult, BatchError> {
    let mut successes: Vec<BatchSuccess> = Vec::new();
    let mut failures: Vec<VerificationError> = Vec::new();
    for parsed_contract in &compilation_result.parsed_contracts {
        let convert_error = |err| {
            VerificationError::new(
                parsed_contract.file_name.clone(),
                parsed_contract.contract_name.clone(),
                VerificationErrorKind::InternalError(format!("{err:#}")),
            )
        };

        let (does_creation_match, creation_values, creation_transformations) = match &contract
            .creation_code
        {
            Some(contract_code) => {
                match transformations::process_creation_code(
                    contract_code,
                    parsed_contract.creation_code.to_vec(),
                    serde_json::to_value(parsed_contract.creation_code_artifacts.clone()).unwrap(),
                )
                .context("process creation code")
                {
                    Ok((processed_code, values, transformations)) => {
                        (&processed_code == contract_code, values, transformations)
                    }
                    Err(err) => {
                        failures.push(convert_error(err));
                        continue;
                    }
                }
            }
            None => (false, Default::default(), Default::default()),
        };

        let (does_runtime_match, runtime_values, runtime_transformations) = match &contract
            .runtime_code
        {
            Some(contract_code) => {
                match transformations::process_runtime_code(
                    contract_code,
                    parsed_contract.runtime_code.to_vec(),
                    serde_json::to_value(parsed_contract.runtime_code_artifacts.clone()).unwrap(),
                )
                .context("process runtime code")
                {
                    Ok((processed_code, values, transformations)) => {
                        (&processed_code == contract_code, values, transformations)
                    }
                    Err(err) => {
                        failures.push(convert_error(err));
                        continue;
                    }
                }
            }
            None => (false, Default::default(), Default::default()),
        };

        if !does_creation_match && !does_runtime_match {
            failures.push(VerificationError::new(
                parsed_contract.file_name.clone(),
                parsed_contract.contract_name.clone(),
                VerificationErrorKind::CodeMismatch,
            ));

            continue;
        }

        let success = BatchSuccess {
            creation_code: parsed_contract.creation_code.to_vec(),
            runtime_code: parsed_contract.runtime_code.to_vec(),
            compiler: compilation_result.compiler.clone(),
            compiler_version: compilation_result.compiler_version.clone(),
            language: compilation_result.language.clone(),
            file_name: parsed_contract.file_name.clone(),
            contract_name: parsed_contract.contract_name.clone(),
            sources: compilation_result.sources.clone(),
            compiler_settings: compilation_result.compiler_settings.clone(),
            compilation_artifacts: serde_json::to_value(
                parsed_contract.compilation_artifacts.clone(),
            )
            .expect("is json serializable"),
            creation_code_artifacts: serde_json::to_value(
                parsed_contract.creation_code_artifacts.clone(),
            )
            .expect("is json serializable"),
            runtime_code_artifacts: serde_json::to_value(
                parsed_contract.runtime_code_artifacts.clone(),
            )
            .expect("is json serializable"),
            creation_match: does_creation_match.then_some(Match {
                values: creation_values,
                transformations: creation_transformations,
            }),
            runtime_match: does_runtime_match.then_some(Match {
                values: runtime_values,
                transformations: runtime_transformations,
            }),
        };

        successes.push(success);
    }

    match choose_best_contract(successes) {
        Some(success) => Ok(VerificationResult::Success(success)),
        None => Ok(VerificationResult::Failure(failures)),
    }
}

fn choose_best_contract(successes: Vec<BatchSuccess>) -> Option<BatchSuccess> {
    if successes.is_empty() {
        return None;
    }

    let mut best_contract = BatchSuccess::default();
    for success in successes {
        if best_contract.creation_match.is_some() && best_contract.runtime_match.is_some() {
            return Some(best_contract);
        }

        if success.creation_match.is_some() && success.runtime_match.is_some() {
            best_contract = success;
            continue;
        }

        if success.creation_match.is_some() && best_contract.creation_match.is_none() {
            best_contract = success;
            continue;
        }

        if success.runtime_match.is_some()
            && best_contract.creation_match.is_none()
            && best_contract.runtime_match.is_none()
        {
            best_contract = success;
            continue;
        }
    }

    Some(best_contract)
}

struct ContractToParse {
    file_name: String,
    contract_name: String,
    contract: lossless_compiler_output::Contract,
    modified_contract: lossless_compiler_output::Contract,
    source_files: lossless_compiler_output::SourceFiles,
}

impl ContractToParse {
    pub fn parse(self) -> Result<ParsedSolidityContract, BatchError> {
        let (creation_code, creation_cbor_auxdata) = self.parse_creation_code_details()?;
        let (runtime_code, runtime_cbor_auxdata) = self.parse_runtime_code_details()?;

        let compilation_artifacts =
            super::artifacts::compilation_artifacts::generate(&self.contract, &self.source_files);
        let creation_code_artifacts = super::artifacts::creation_code_artifacts::generate(
            &self.contract,
            creation_cbor_auxdata,
        );
        let runtime_code_artifacts = super::artifacts::runtime_code_artifacts::generate(
            &self.contract,
            runtime_cbor_auxdata,
        );

        Ok(ParsedSolidityContract {
            _contract: self.contract,
            file_name: self.file_name,
            contract_name: self.contract_name,
            compilation_artifacts,
            creation_code,
            creation_code_artifacts,
            runtime_code,
            runtime_code_artifacts,
        })
    }

    pub fn parse_creation_code_details(&self) -> Result<(Bytes, CborAuxdata), BatchError> {
        let code = self.preprocess_code(&self.contract.evm.bytecode, "creation")?;
        let modified_code =
            self.preprocess_code(&self.modified_contract.evm.bytecode, "modified creation")?;

        let cbor_auxdata = split(&self.file_name, &self.contract_name, &code, &modified_code)?;

        Ok((code, cbor_auxdata))
    }

    pub fn parse_runtime_code_details(&self) -> Result<(Bytes, CborAuxdata), BatchError> {
        let code =
            self.preprocess_code(&self.contract.evm.deployed_bytecode.bytecode, "deployed")?;
        let modified_code = self.preprocess_code(
            &self.modified_contract.evm.deployed_bytecode.bytecode,
            "modified deployed",
        )?;

        let cbor_auxdata = split(&self.file_name, &self.contract_name, &code, &modified_code)?;

        Ok((code, cbor_auxdata))
    }

    fn preprocess_code(
        &self,
        code_bytecode: &lossless_compiler_output::Bytecode,
        prefix: impl Into<String>,
    ) -> Result<Bytes, BatchError> {
        let prefix = get_prefix(prefix);
        let code_link_references = code_bytecode
            .link_references
            .as_ref()
            .map(|references| serde_json::from_value::<LinkReferences>(references.clone()))
            .transpose()
            .map_err(|err| {
                internal_error(
                    &self.file_name,
                    Some(&self.contract_name),
                    &format!("deserializing {prefix}code link references failed: {err}"),
                )
            })?
            .unwrap_or_default();
        let code = match code_bytecode.object.clone() {
            foundry_compilers::artifacts::BytecodeObject::Bytecode(bytes) => bytes.0,
            foundry_compilers::artifacts::BytecodeObject::Unlinked(value) => nullify_libraries(
                &self.file_name,
                &self.contract_name,
                value,
                code_link_references,
            )?,
        };
        Ok(code)
    }
}

fn parse_contracts(
    compiler_version: Version,
    compiler_input: &foundry_compilers::CompilerInput,
    compiler_output: serde_json::Value,
    modified_compiler_output: serde_json::Value,
) -> Result<CompilationResult, BatchError> {
    let compiler_output = to_lossless_output(compiler_output, "")?;
    let modified_compiler_output = to_lossless_output(modified_compiler_output, "modified")?;

    let mut parsed_contracts = Vec::new();
    // Here we are re-using the fact that BTreeMaps::into_iter
    // produces items in order by key.
    for ((file_name, contracts), (modified_file_name, modified_contracts)) in compiler_output
        .contracts
        .into_iter()
        .zip(modified_compiler_output.contracts)
    {
        if file_name != modified_file_name {
            return Err(internal_error(
                &file_name,
                None,
                &format!(
                    "modified file name does not correspond to original one: {modified_file_name}"
                ),
            ));
        }

        for ((contract_name, contract), (modified_contract_name, modified_contract)) in
            contracts.into_iter().zip(modified_contracts)
        {
            if contract_name != modified_contract_name {
                return Err(internal_error(
                    &file_name,
                    Some(&contract_name),
                    &format!(
                        "modified contract name does not correspond to original one: {modified_contract_name}"
                    )
                ));
            }
            parsed_contracts.push(
                ContractToParse {
                    file_name: file_name.clone(),
                    contract_name,
                    contract,
                    modified_contract,
                    source_files: compiler_output.sources.clone(),
                }
                .parse()?,
            );
        }
    }

    Ok(CompilationResult {
        compiler: "SOLC".to_string(),
        compiler_version: compiler_version.to_string(),
        language: compiler_input.language.clone().to_uppercase(),
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

fn to_lossless_output(
    raw: serde_json::Value,
    prefix: impl Into<String>,
) -> Result<lossless_compiler_output::CompilerOutput, BatchError> {
    serde_json::from_value(raw).map_err(|err| {
        let prefix = get_prefix(prefix);
        tracing::error!("cannot parse {prefix}compiler output in lossless format: {err}");
        BatchError::Internal(anyhow::anyhow!(
            "cannot parse {prefix}compiler output in lossless format: {err}"
        ))
    })
}

fn get_prefix(prefix: impl Into<String>) -> String {
    let mut prefix: String = prefix.into();
    if prefix.is_empty() {
        prefix = " ".into();
    }
    prefix
}

fn nullify_libraries(
    file_name: &str,
    contract_name: &str,
    mut code: String,
    link_references: LinkReferences,
) -> Result<Bytes, BatchError> {
    let offsets = link_references
        .into_values()
        .flat_map(|file_references| file_references.into_values())
        .flatten();
    for offset in offsets {
        // Offset stores start and length values for bytes, while code is a hex encoded string
        let start = offset.start as usize * 2;
        let length = offset.length as usize * 2;
        if code.len() < start + length {
            return Err(internal_error(
                file_name,
                Some(contract_name),
                "link reference offset exceeds code size",
            ));
        }

        code.replace_range(start..start + length, "0".repeat(length).as_str());
    }

    let result = decode_hex(&code).map_err(|err| {
        internal_error(
            file_name,
            Some(contract_name),
            &format!("cannot format bytecode as bytes {err}"),
        )
    })?;

    Ok(Bytes::from(result))
}

/// Splits bytecode onto [`BytecodePart`]s using bytecode with modified metadata hashes.
///
/// Any error here is [`crate::verifier::errors::VerificationErrorKind::InternalError`], as both original
/// and modified bytecodes are obtained as a result of local compilation.
fn split(
    file_name: &str,
    contract_name: &str,
    raw: &Bytes,
    raw_modified: &Bytes,
) -> Result<CborAuxdata, BatchError> {
    if raw.len() != raw_modified.len() {
        return Err(internal_error(
            file_name,
            Some(contract_name),
            &format!(
                "bytecode and modified bytecode length mismatch: {}",
                Mismatch::new(raw.len(), raw_modified.len())
            ),
        ));
    }

    let parts_total_size =
        |parts: &Vec<BytecodePart>| -> usize { parts.iter().fold(0, |size, el| size + el.size()) };

    let mut bytecode_parts = Vec::new();

    let mut i = 0usize;
    while i < raw.len() {
        let decoded = parse_bytecode_parts(
            file_name,
            contract_name,
            &raw.slice(i..raw.len()),
            &raw_modified[i..],
        )?;
        let decoded_size = parts_total_size(&decoded);
        bytecode_parts.extend(decoded);

        i += decoded_size;
    }

    Ok(cbor_auxdata::generate(&bytecode_parts))
}

/// Finds the next [`BytecodePart`]s into a series of bytes.
///
/// Parses at most one [`BytecodePart::Main`] and one [`BytecodePart::Metadata`].
fn parse_bytecode_parts(
    file_name: &str,
    contract_name: &str,
    raw: &Bytes,
    raw_modified: &[u8],
) -> Result<Vec<BytecodePart>, BatchError> {
    let mut parts = Vec::new();

    let len = raw.len();

    // search for the first non-matching byte
    let mut index = raw
        .iter()
        .zip(raw_modified.iter())
        .position(|(a, b)| a != b);

    // There is some non-matching byte - part of the metadata part byte.
    if let Some(mut i) = index {
        let is_metadata_length_valid = |metadata_length: usize, i: usize| {
            if len < i + metadata_length + 2 {
                return false;
            }

            // Decode length of metadata hash representation
            let mut metadata_length_raw =
                raw.slice((i + metadata_length)..(i + metadata_length + 2));
            let encoded_metadata_length = metadata_length_raw.get_u16() as usize;
            if encoded_metadata_length != metadata_length {
                return false;
            }

            true
        };

        // `i` is the first different byte. The metadata hash itself started somewhere earlier
        // (at least for "a1"/"a2" indicating number of elements in cbor mapping).
        // Next steps are trying to find that beginning.

        let (metadata, metadata_length) = loop {
            let mut result = MetadataHash::from_cbor(&raw[i..]);
            while result.is_err() {
                // It is the beginning of the bytecode segment but no metadata hash has been parsed
                if i == 0 {
                    return Err(internal_error(
                        file_name,
                        Some(contract_name),
                        "failed to parse bytecode part",
                    ));
                }
                i -= 1;

                result = MetadataHash::from_cbor(&raw[i..]);
            }

            let (metadata, metadata_length) = result.unwrap();

            if is_metadata_length_valid(metadata_length, i) {
                break (metadata, metadata_length);
            }

            i -= 1;
        };

        parts.push(BytecodePart::Metadata {
            raw: raw.slice(i..(i + metadata_length + 2)),
            metadata,
        });

        // Update index to point where metadata part begins
        index = Some(i);
    }

    // If there is something before metadata part (if any)
    // belongs to main part
    let i = index.unwrap_or(len);
    if i > 0 {
        parts.insert(
            0,
            BytecodePart::Main {
                raw: raw.slice(0..i),
            },
        )
    }

    Ok(parts)
}

fn internal_error(file_name: &str, contract_name: Option<&str>, message: &str) -> BatchError {
    match contract_name {
        None => {
            tracing::error!(file = file_name, message);
            BatchError::Internal(anyhow::anyhow!("file: {file_name} - {message}"))
        }
        Some(contract_name) => {
            tracing::error!(file = file_name, contract = contract_name, message);
            BatchError::Internal(anyhow::anyhow!(
                "file: {file_name}; contract: {contract_name} - {message}"
            ))
        }
    }
}
