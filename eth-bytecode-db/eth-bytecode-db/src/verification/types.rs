use super::smart_contract_verifier;
use crate::FromHex;
use anyhow::Context;
use entity::sea_orm_active_enums;
use eth_bytecode_db_proto::blockscout::eth_bytecode_db::v2 as eth_bytecode_db_v2;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, str::FromStr};

/********** Bytecode Part **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BytecodePart {
    Main { data: Vec<u8> },
    Meta { data: Vec<u8> },
}

impl TryFrom<smart_contract_verifier::BytecodePart> for BytecodePart {
    type Error = anyhow::Error;

    fn try_from(value: smart_contract_verifier::BytecodePart) -> Result<Self, Self::Error> {
        let data = hex::decode(value.data.trim_start_matches("0x"))?;
        match value.r#type.as_str() {
            "main" => Ok(Self::Main { data }),
            "meta" => Ok(Self::Meta { data }),
            _ => Err(anyhow::anyhow!("Unknown type")),
        }
    }
}

impl From<&BytecodePart> for sea_orm_active_enums::PartType {
    fn from(value: &BytecodePart) -> Self {
        match value {
            BytecodePart::Main { .. } => Self::Main,
            BytecodePart::Meta { .. } => Self::Metadata,
        }
    }
}

impl BytecodePart {
    pub fn data(&self) -> &[u8] {
        match self {
            BytecodePart::Main { data } => data.as_ref(),
            BytecodePart::Meta { data } => data.as_ref(),
        }
    }

    pub fn data_owned(self) -> Vec<u8> {
        match self {
            BytecodePart::Main { data } => data,
            BytecodePart::Meta { data } => data,
        }
    }
}

/********** Bytecode Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BytecodeType {
    CreationInput,
    DeployedBytecode,
}

impl From<BytecodeType> for sea_orm_active_enums::BytecodeType {
    fn from(value: BytecodeType) -> Self {
        match value {
            BytecodeType::CreationInput => sea_orm_active_enums::BytecodeType::CreationInput,
            BytecodeType::DeployedBytecode => sea_orm_active_enums::BytecodeType::DeployedBytecode,
        }
    }
}

impl From<BytecodeType> for smart_contract_verifier::BytecodeType {
    fn from(value: BytecodeType) -> Self {
        match value {
            BytecodeType::CreationInput => smart_contract_verifier::BytecodeType::CreationInput,
            BytecodeType::DeployedBytecode => {
                smart_contract_verifier::BytecodeType::DeployedBytecode
            }
        }
    }
}

/********** Source Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceType {
    Solidity,
    Vyper,
    Yul,
}

impl From<SourceType> for sea_orm_active_enums::SourceType {
    fn from(source_type: SourceType) -> Self {
        match source_type {
            SourceType::Solidity => sea_orm_active_enums::SourceType::Solidity,
            SourceType::Vyper => sea_orm_active_enums::SourceType::Vyper,
            SourceType::Yul => sea_orm_active_enums::SourceType::Yul,
        }
    }
}

impl From<sea_orm_active_enums::SourceType> for SourceType {
    fn from(source_type: sea_orm_active_enums::SourceType) -> Self {
        match source_type {
            sea_orm_active_enums::SourceType::Solidity => SourceType::Solidity,
            sea_orm_active_enums::SourceType::Vyper => SourceType::Vyper,
            sea_orm_active_enums::SourceType::Yul => SourceType::Yul,
        }
    }
}

impl TryFrom<smart_contract_verifier::SourceType> for SourceType {
    type Error = anyhow::Error;

    fn try_from(value: smart_contract_verifier::SourceType) -> Result<Self, Self::Error> {
        match value {
            smart_contract_verifier::SourceType::Unspecified => {
                Err(anyhow::anyhow!("Unknown type: {}", value.as_str_name()))
            }
            smart_contract_verifier::SourceType::Solidity => Ok(SourceType::Solidity),
            smart_contract_verifier::SourceType::Vyper => Ok(SourceType::Vyper),
            smart_contract_verifier::SourceType::Yul => Ok(SourceType::Yul),
        }
    }
}

impl From<SourceType> for smart_contract_verifier::SourceType {
    fn from(value: SourceType) -> Self {
        match value {
            SourceType::Solidity => smart_contract_verifier::SourceType::Solidity,
            SourceType::Vyper => smart_contract_verifier::SourceType::Vyper,
            SourceType::Yul => smart_contract_verifier::SourceType::Yul,
        }
    }
}

/********** Match Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchType {
    Unknown,
    Partial,
    Full,
}

impl From<smart_contract_verifier::MatchType> for MatchType {
    fn from(value: smart_contract_verifier::MatchType) -> Self {
        match value {
            smart_contract_verifier::MatchType::Unspecified => MatchType::Unknown,
            smart_contract_verifier::MatchType::Partial => MatchType::Partial,
            smart_contract_verifier::MatchType::Full => MatchType::Full,
        }
    }
}

/********** Source **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: String,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<String>,
    pub constructor_arguments: Option<String>,
    pub match_type: MatchType,
    pub compilation_artifacts: Option<String>,
    pub creation_input_artifacts: Option<String>,
    pub deployed_bytecode_artifacts: Option<String>,

    pub raw_creation_input: Vec<u8>,
    pub raw_deployed_bytecode: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl
    TryFrom<(
        smart_contract_verifier::Source,
        smart_contract_verifier::ExtraData,
    )> for Source
{
    type Error = anyhow::Error;

    fn try_from(
        (source, extra_data): (
            smart_contract_verifier::Source,
            smart_contract_verifier::ExtraData,
        ),
    ) -> Result<Self, Self::Error> {
        let parse_local_parts = |local_parts: Vec<smart_contract_verifier::BytecodePart>,
                                 bytecode_type: &str|
         -> Result<(Vec<BytecodePart>, Vec<u8>), anyhow::Error> {
            let parts = local_parts
                .into_iter()
                .map(|part| {
                    BytecodePart::try_from(part).map_err(|err| {
                        anyhow::anyhow!("error while decoding local {}: {}", bytecode_type, err,)
                            .context("verifier service connection")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;

            let raw_input = parts
                .iter()
                .flat_map(|part| part.data().to_vec())
                .collect::<Vec<_>>();

            Ok((parts, raw_input))
        };

        let (creation_input_parts, raw_creation_input) =
            parse_local_parts(extra_data.local_creation_input_parts, "creation input")?;
        let (deployed_bytecode_parts, raw_deployed_bytecode) = parse_local_parts(
            extra_data.local_deployed_bytecode_parts,
            "deployed bytecode",
        )?;

        let source_type = source.source_type().try_into()?;
        let match_type = source.match_type().into();
        Ok(Self {
            file_name: source.file_name,
            contract_name: source.contract_name,
            compiler_version: source.compiler_version,
            compiler_settings: source.compiler_settings,
            source_type,
            source_files: source.source_files,
            abi: source.abi,
            constructor_arguments: source.constructor_arguments,
            match_type,
            compilation_artifacts: source.compilation_artifacts,
            creation_input_artifacts: source.creation_input_artifacts,
            deployed_bytecode_artifacts: source.deployed_bytecode_artifacts,
            raw_creation_input,
            raw_deployed_bytecode,
            creation_input_parts,
            deployed_bytecode_parts,
        })
    }
}

/// The same as [`Source`] but processed to be inserted into the database.
/// The processing consists of converting all JSON stored types into [`serde_json::Value`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatabaseReadySource {
    pub file_name: String,
    pub contract_name: String,
    pub compiler_version: String,
    pub compiler_settings: serde_json::Value,
    pub source_type: SourceType,
    pub source_files: BTreeMap<String, String>,
    pub abi: Option<serde_json::Value>,
    pub compilation_artifacts: Option<serde_json::Value>,
    pub creation_code_artifacts: Option<serde_json::Value>,
    pub runtime_code_artifacts: Option<serde_json::Value>,

    pub raw_creation_code: Vec<u8>,
    pub raw_runtime_code: Vec<u8>,
    pub creation_input_parts: Vec<BytecodePart>,
    pub deployed_bytecode_parts: Vec<BytecodePart>,
}

impl TryFrom<Source> for DatabaseReadySource {
    type Error = anyhow::Error;

    fn try_from(value: Source) -> Result<Self, Self::Error> {
        let abi = value
            .abi
            .map(|abi| serde_json::from_str(&abi).context("deserialize abi into json value"))
            .transpose()?;
        let compiler_settings: serde_json::Value =
            serde_json::from_str(&value.compiler_settings)
                .context("deserialize compiler settings into json value")?;
        let compilation_artifacts: Option<serde_json::Value> = value
            .compilation_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize compilation artifacts into json value")?;
        let creation_input_artifacts: Option<serde_json::Value> = value
            .creation_input_artifacts
            .as_deref()
            .map(serde_json::from_str)
            .transpose()
            .context("deserialize creation input artifacts into json value")?;
        let deployed_bytecode_artifacts: Option<serde_json::Value> = value
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

impl TryFrom<AllianceContractImportSuccess> for DatabaseReadySource {
    type Error = anyhow::Error;

    fn try_from(value: AllianceContractImportSuccess) -> Result<Self, Self::Error> {
        let source_type = match value.language {
            Language::Solidity => SourceType::Solidity,
            Language::Yul => SourceType::Yul,
            Language::Vyper => SourceType::Vyper,
        };

        #[derive(Deserialize)]
        struct CompilationArtifacts {
            pub abi: Option<serde_json::Value>,
        }
        let abi =
            serde_json::from_value::<CompilationArtifacts>(value.compilation_artifacts.clone())
                .context("extractor abi json from compilation artifacts")?
                .abi;

        let creation_code_parts = code_parts(
            value.creation_code.clone(),
            value.creation_code_artifacts.clone(),
        )?;
        let runtime_code_parts = code_parts(
            value.runtime_code.clone(),
            value.runtime_code_artifacts.clone(),
        )?;

        Ok(Self {
            file_name: value.file_name,
            contract_name: value.contract_name,
            compiler_version: value.compiler_version,
            compiler_settings: value.compiler_settings,
            source_type,
            source_files: value.sources,
            abi,
            compilation_artifacts: Some(value.compilation_artifacts),
            creation_code_artifacts: Some(value.creation_code_artifacts),
            runtime_code_artifacts: Some(value.runtime_code_artifacts),
            raw_creation_code: value.creation_code.to_vec(),
            raw_runtime_code: value.runtime_code.to_vec(),
            creation_input_parts: creation_code_parts,
            deployed_bytecode_parts: runtime_code_parts,
        })
    }
}

fn code_parts(
    code: bytes::Bytes,
    code_artifacts: serde_json::Value,
) -> Result<Vec<BytecodePart>, anyhow::Error> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CborAuxdata {
        pub offset: usize,
        #[serde(deserialize_with = "crate::deserialize_bytes")]
        pub value: bytes::Bytes,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct CodeArtifacts {
        #[serde(default)]
        pub cbor_auxdata: BTreeMap<String, CborAuxdata>,
    }

    let code_artifacts: CodeArtifacts =
        serde_json::from_value(code_artifacts).context("code artifacts deserialization")?;

    let mut parts = vec![];

    let mut i = 0usize;
    let mut cbor_auxdata = code_artifacts
        .cbor_auxdata
        .into_values()
        .collect::<Vec<_>>();
    cbor_auxdata.sort_by_key(|v| v.offset);
    for auxdata in cbor_auxdata {
        parts.push(BytecodePart::Main {
            data: code[i..auxdata.offset].to_vec(),
        });
        parts.push(BytecodePart::Meta {
            data: auxdata.value.to_vec(),
        });
        i = auxdata.offset + auxdata.value.len();
    }

    if i < code.len() {
        parts.push(BytecodePart::Main {
            data: code[i..].to_vec(),
        });
    }

    Ok(parts)
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_code_parts() {
        // let code_parts = serde_json
    }
}

/********** Verification Request **********/

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationMetadata {
    pub chain_id: Option<i64>,
    pub contract_address: Option<bytes::Bytes>,
    pub transaction_hash: Option<bytes::Bytes>,
    pub block_number: Option<i64>,
    pub transaction_index: Option<i64>,
    pub deployer: Option<bytes::Bytes>,
    pub creation_code: Option<bytes::Bytes>,
    pub runtime_code: Option<bytes::Bytes>,
}

impl From<VerificationMetadata> for smart_contract_verifier::VerificationMetadata {
    fn from(value: VerificationMetadata) -> Self {
        let chain_id = value.chain_id.map(|id| format!("{}", id));
        let contract_address = value
            .contract_address
            .map(|address| blockscout_display_bytes::Bytes::from(address).to_string());
        Self {
            chain_id,
            contract_address,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct VerificationRequest<T> {
    pub bytecode: String,
    pub bytecode_type: BytecodeType,
    pub compiler_version: String,
    #[serde(flatten)]
    pub content: T,
    pub metadata: Option<VerificationMetadata>,
    #[serde(skip_serializing)]
    pub is_authorized: bool,
}

/********** Verifier Alliance Import Request **********/

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AllianceContract {
    pub chain_id: String,
    pub contract_address: bytes::Bytes,
    pub transaction_hash: Option<bytes::Bytes>,
    pub block_number: Option<i64>,
    pub transaction_index: Option<i64>,
    pub deployer: Option<bytes::Bytes>,
    pub creation_code: Option<bytes::Bytes>,
    pub runtime_code: bytes::Bytes,
}

impl TryFrom<eth_bytecode_db_v2::VerifierAllianceContract> for AllianceContract {
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(value: eth_bytecode_db_v2::VerifierAllianceContract) -> Result<Self, Self::Error> {
        let str_to_bytes = |value: &str| {
            FromHex::from_hex(value)
                .map_err(|v| eth_bytecode_db_proto::tonic::Status::invalid_argument(v.to_string()))
        };

        Ok(Self {
            chain_id: value.chain_id,
            contract_address: str_to_bytes(&value.contract_address)?,
            transaction_hash: value
                .transaction_hash
                .as_deref()
                .map(str_to_bytes)
                .transpose()?,
            block_number: value.block_number,
            transaction_index: value.transaction_index,
            deployer: value.deployer.as_deref().map(str_to_bytes).transpose()?,
            creation_code: value
                .creation_code
                .as_deref()
                .map(str_to_bytes)
                .transpose()?,
            runtime_code: str_to_bytes(&value.runtime_code)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AllianceImportRequest<T> {
    pub contracts: Vec<AllianceContract>,
    pub compiler_version: String,
    #[serde(flatten)]
    pub content: T,
}

/********** Verifier Alliance Import Result **********/

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Compiler {
    Solc,
    Vyper,
}

impl TryFrom<smart_contract_verifier::contract_verification_success::compiler::Compiler>
    for Compiler
{
    type Error = crate::verification::Error;

    fn try_from(
        value: smart_contract_verifier::contract_verification_success::compiler::Compiler,
    ) -> Result<Self, Self::Error> {
        match value {
            smart_contract_verifier::contract_verification_success::compiler::Compiler::Solc => Ok(Compiler::Solc),
            smart_contract_verifier::contract_verification_success::compiler::Compiler::Vyper => Ok(Compiler::Vyper),
            smart_contract_verifier::contract_verification_success::compiler::Compiler::Unspecified => {
                Err(crate::verification::Error::Verifier(anyhow::anyhow!("compiler is unspecified")))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Language {
    Solidity,
    Yul,
    Vyper,
}

impl TryFrom<smart_contract_verifier::contract_verification_success::language::Language>
    for Language
{
    type Error = crate::verification::Error;

    fn try_from(
        value: smart_contract_verifier::contract_verification_success::language::Language,
    ) -> Result<Self, Self::Error> {
        match value {
            smart_contract_verifier::contract_verification_success::language::Language::Solidity => Ok(Language::Solidity),
            smart_contract_verifier::contract_verification_success::language::Language::Yul => Ok(Language::Yul),
            smart_contract_verifier::contract_verification_success::language::Language::Vyper => Ok(Language::Vyper),
            smart_contract_verifier::contract_verification_success::language::Language::Unspecified =>
                Err(crate::verification::Error::Verifier(anyhow::anyhow!("language is unspecified")))
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchDetails {
    pub match_type: MatchType,
    pub values: serde_json::Value,
    pub transformations: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AllianceContractImportSuccess {
    pub creation_code: bytes::Bytes,
    pub runtime_code: bytes::Bytes,
    pub compiler: Compiler,
    pub compiler_version: String,
    pub language: Language,
    pub file_name: String,
    pub contract_name: String,
    pub sources: BTreeMap<String, String>,
    pub compiler_settings: serde_json::Value,
    pub compilation_artifacts: serde_json::Value,
    pub creation_code_artifacts: serde_json::Value,
    pub runtime_code_artifacts: serde_json::Value,
    pub creation_match_details: Option<MatchDetails>,
    pub runtime_match_details: Option<MatchDetails>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AllianceContractImportResult {
    Success(AllianceContractImportSuccess),
    VerificationFailure {},
    ImportFailure(String),
}

impl TryFrom<smart_contract_verifier::ContractVerificationResult> for AllianceContractImportResult {
    type Error = crate::verification::Error;

    fn try_from(
        value: smart_contract_verifier::ContractVerificationResult,
    ) -> Result<Self, Self::Error> {
        let str_to_bytes = |value: &str| {
            FromHex::from_hex(value).map_err(|err| Self::Error::Verifier(anyhow::anyhow!("{err}")))
        };

        let str_to_value = |value: &str| {
            serde_json::Value::from_str(value)
                .map_err(|err| Self::Error::Verifier(anyhow::anyhow!("{err}")))
        };

        let parse_match_details = |details: smart_contract_verifier::contract_verification_success::MatchDetails|
         -> Result<MatchDetails, Self::Error> {
            let match_type = match details.match_type() {
                smart_contract_verifier::contract_verification_success::MatchType::Undefined => MatchType::Unknown,
                smart_contract_verifier::contract_verification_success::MatchType::Partial => MatchType::Partial,
                smart_contract_verifier::contract_verification_success::MatchType::Full => MatchType::Full,
            };

            Ok(MatchDetails {
                match_type,
                values: str_to_value(&details.values)?,
                transformations: str_to_value(&details.transformations)?,
            })
        };

        let result = match value {
            smart_contract_verifier::ContractVerificationResult {
                verification_result: Some(smart_contract_verifier::contract_verification_result::VerificationResult::Success(value))
            } => {

                let compiler = value.compiler();
                let language = value.language();
                Self::Success(AllianceContractImportSuccess {
                    creation_code: str_to_bytes(&value.creation_code)?,
                    runtime_code: str_to_bytes(&value.runtime_code)?,
                    compiler: compiler.try_into()?,
                    compiler_version: value.compiler_version,
                    language: language.try_into()?,
                    file_name: value.file_name,
                    contract_name: value.contract_name,
                    sources: value.sources,
                    compiler_settings: str_to_value(&value.compiler_settings)?,
                    compilation_artifacts: str_to_value(&value.compilation_artifacts)?,
                    creation_code_artifacts: str_to_value(&value.creation_code_artifacts)?,
                    runtime_code_artifacts: str_to_value(&value.runtime_code_artifacts)?,
                    creation_match_details: value.creation_match_details.map(parse_match_details).transpose()?,
                    runtime_match_details: value.runtime_match_details.map(parse_match_details).transpose()?,
                })
            }
            smart_contract_verifier::ContractVerificationResult {
                verification_result: Some(smart_contract_verifier::contract_verification_result::VerificationResult::Failure(_value))
            } => {
                Self::VerificationFailure {}
            }
            value => return Err(crate::verification::Error::Verifier(
                anyhow::anyhow!("invalid struct: {value:?}"))
            )
        };

        Ok(result)
    }
}

impl TryFrom<AllianceContractImportResult>
    for eth_bytecode_db_v2::verifier_alliance_batch_import_response::ImportContractResult
{
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(value: AllianceContractImportResult) -> Result<Self, Self::Error> {
        let result = match value {
            AllianceContractImportResult::Success(success) => {
                eth_bytecode_db_v2::verifier_alliance_batch_import_response::import_contract_result::Result::Success(
                    eth_bytecode_db_v2::verifier_alliance_batch_import_response::Success {
                        creation_code_match_type: match_details_to_proto_match_type(success.creation_match_details.as_ref()).into(),
                        runtime_code_match_type: match_details_to_proto_match_type(success.runtime_match_details.as_ref()).into(),
                    }
                )
            }
            AllianceContractImportResult::VerificationFailure {} => eth_bytecode_db_v2::verifier_alliance_batch_import_response::import_contract_result::Result::VerificationFailure(
                eth_bytecode_db_v2::verifier_alliance_batch_import_response::VerificationFailure {}
            ),
            AllianceContractImportResult::ImportFailure(_message) => eth_bytecode_db_v2::verifier_alliance_batch_import_response::import_contract_result::Result::ImportFailure(
                eth_bytecode_db_v2::verifier_alliance_batch_import_response::ImportFailure {}
            )
        };

        Ok(Self {
            result: Some(result),
        })
    }
}

fn match_details_to_proto_match_type(
    details: Option<&MatchDetails>,
) -> eth_bytecode_db_v2::verifier_alliance_batch_import_response::MatchType {
    match details {
        None => eth_bytecode_db_v2::verifier_alliance_batch_import_response::MatchType::NoMatch,
        Some(MatchDetails {
            match_type: MatchType::Unknown,
            ..
        }) => eth_bytecode_db_v2::verifier_alliance_batch_import_response::MatchType::NotDefined,
        Some(MatchDetails {
            match_type: MatchType::Partial,
            ..
        }) => eth_bytecode_db_v2::verifier_alliance_batch_import_response::MatchType::Partial,
        Some(MatchDetails {
            match_type: MatchType::Full,
            ..
        }) => eth_bytecode_db_v2::verifier_alliance_batch_import_response::MatchType::Full,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AllianceBatchImportResult {
    CompilationFailure(String),
    Results(Vec<AllianceContractImportResult>),
}

impl TryFrom<smart_contract_verifier::BatchVerifyResponse> for AllianceBatchImportResult {
    type Error = crate::verification::Error;

    fn try_from(value: smart_contract_verifier::BatchVerifyResponse) -> Result<Self, Self::Error> {
        let result = match value {
            smart_contract_verifier::BatchVerifyResponse {
                verification_result: Some(smart_contract_verifier::batch_verify_response::VerificationResult::CompilationFailure(
                smart_contract_verifier::CompilationFailure { message }
                                          ))
            } => AllianceBatchImportResult::CompilationFailure(message),
            smart_contract_verifier::BatchVerifyResponse {
                verification_result: Some(smart_contract_verifier::batch_verify_response::VerificationResult::ContractVerificationResults(
                    smart_contract_verifier::batch_verify_response::ContractVerificationResults {
                        items
                    }))
            } => {
                let results = items.into_iter().map(TryFrom::try_from).collect::<Result<_, _>>()?;
                AllianceBatchImportResult::Results(results)
            },
            value => return Err(crate::verification::Error::Verifier(
                anyhow::anyhow!("invalid struct: {value:?}"))
            )
        };

        Ok(result)
    }
}

impl TryFrom<AllianceBatchImportResult>
    for eth_bytecode_db_v2::VerifierAllianceBatchImportResponse
{
    type Error = eth_bytecode_db_proto::tonic::Status;

    fn try_from(value: AllianceBatchImportResult) -> Result<Self, Self::Error> {
        let result = match value {
            AllianceBatchImportResult::CompilationFailure(message) => {
                eth_bytecode_db_v2::verifier_alliance_batch_import_response::Response::CompilationFailure(
                    eth_bytecode_db_v2::verifier_alliance_batch_import_response::CompilationFailure {
                        message
                    }
                )
            }
            AllianceBatchImportResult::Results(results) => {
                eth_bytecode_db_v2::verifier_alliance_batch_import_response::Response::ImportResults(
                    eth_bytecode_db_v2::verifier_alliance_batch_import_response::ImportContractResults {
                        items: results.into_iter().map(TryFrom::try_from).collect::<Result<_, _>>()?,
                    }
                )
            }
        };

        Ok(eth_bytecode_db_v2::VerifierAllianceBatchImportResponse {
            response: Some(result),
        })
    }
}

/********** Verification Type **********/

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum VerificationType {
    MultiPartFiles,
    StandardJson,
}

impl From<VerificationType> for sea_orm_active_enums::VerificationType {
    fn from(value: VerificationType) -> Self {
        match value {
            VerificationType::MultiPartFiles => {
                sea_orm_active_enums::VerificationType::MultiPartFiles
            }
            VerificationType::StandardJson => sea_orm_active_enums::VerificationType::StandardJson,
        }
    }
}
