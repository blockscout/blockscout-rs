use crate::{
    solidity, solidity::RequestParseError, verify_new::SolcInput, OnChainCode, OnChainContract,
};
use anyhow::Context;
use foundry_compilers_new::artifacts::EvmVersion;
use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
    BatchVerifySolidityStandardJsonRequest, Contract, VerifySolidityMultiPartRequest,
    VerifySolidityStandardJsonRequest,
};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

impl TryFrom<VerifySolidityMultiPartRequest> for solidity::multi_part::VerificationRequestNew {
    type Error = RequestParseError;

    fn try_from(request: VerifySolidityMultiPartRequest) -> Result<Self, Self::Error> {
        let on_chain_code = helpers::decode_on_chain_code_from_value_and_type(
            &request.bytecode,
            request.bytecode_type(),
        )?;

        let compiler_version = helpers::decode_compiler_version(&request.compiler_version)?;

        let sources: BTreeMap<PathBuf, String> = request
            .source_files
            .into_iter()
            .map(|(name, content)| (PathBuf::from(name), content))
            .collect();

        let evm_version = match request.evm_version {
            Some(version) if version != "default" => Some(
                EvmVersion::from_str(&version)
                    .map_err(|err| anyhow::anyhow!("invalid evm_version: {err}"))?,
            ),
            _ => None,
        };

        let (chain_id, address) = helpers::decode_verification_metadata(request.metadata);

        Ok(Self {
            on_chain_code,
            compiler_version,
            content: solidity::multi_part::Content {
                sources,
                evm_version,
                optimization_runs: request.optimization_runs.map(|i| i as usize),
                contract_libraries: request.libraries,
            },
            chain_id,
            address,
        })
    }
}

impl TryFrom<VerifySolidityStandardJsonRequest>
    for solidity::standard_json::VerificationRequestNew
{
    type Error = RequestParseError;

    fn try_from(request: VerifySolidityStandardJsonRequest) -> Result<Self, Self::Error> {
        let on_chain_code = helpers::decode_on_chain_code_from_value_and_type(
            &request.bytecode,
            request.bytecode_type(),
        )?;

        let compiler_version = helpers::decode_compiler_version(&request.compiler_version)?;

        let deserializer = &mut serde_json::Deserializer::from_str(&request.input);
        let input: SolcInput = serde_path_to_error::deserialize(deserializer)?;

        let (chain_id, address) = helpers::decode_verification_metadata(request.metadata);

        Ok(Self {
            on_chain_code,
            compiler_version,
            content: input,
            chain_id,
            address,
        })
    }
}

impl TryFrom<Contract> for OnChainContract {
    type Error = RequestParseError;

    fn try_from(value: Contract) -> Result<Self, Self::Error> {
        let runtime_code =
            helpers::decode_optional_hex(value.runtime_code).context("invalid runtime code")?;
        let creation_code =
            helpers::decode_optional_hex(value.creation_code).context("invalid creation code")?;

        let on_chain_code = match (runtime_code, creation_code) {
            (None, None) => Err(anyhow::anyhow!(
                "both runtime and creation code cannot be empty"
            ))?,
            (Some(runtime_code), None) => OnChainCode::runtime(runtime_code),
            (None, Some(creation_code)) => OnChainCode::creation(creation_code),
            (Some(runtime_code), Some(creation_code)) => {
                OnChainCode::complete(runtime_code, creation_code)
            }
        };

        let (chain_id, address) = helpers::decode_verification_metadata(value.metadata);

        Ok(Self {
            code: on_chain_code,
            chain_id,
            address,
        })
    }
}

impl TryFrom<BatchVerifySolidityStandardJsonRequest>
    for solidity::standard_json::BatchVerificationRequestNew
{
    type Error = RequestParseError;

    fn try_from(request: BatchVerifySolidityStandardJsonRequest) -> Result<Self, Self::Error> {
        let contracts = request
            .contracts
            .into_iter()
            .map(OnChainContract::try_from)
            .collect::<Result<Vec<_>, _>>()?;

        let compiler_version = helpers::decode_compiler_version(&request.compiler_version)?;

        let deserializer = &mut serde_json::Deserializer::from_str(&request.input);
        let input: SolcInput = serde_path_to_error::deserialize(deserializer)?;

        Ok(Self {
            contracts,
            compiler_version,
            content: input,
        })
    }
}

mod helpers {
    use crate::{DetailedVersion, OnChainCode};
    use alloy_core::primitives::Address;
    use anyhow::Context;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
        BytecodeType, VerificationMetadata,
    };
    use std::str::FromStr;

    pub fn decode_verification_metadata(
        maybe_value: Option<VerificationMetadata>,
    ) -> (Option<String>, Option<Address>) {
        match maybe_value {
            None => (None, None),
            Some(metadata) => {
                let chain_id = metadata.chain_id;
                let contract_address = metadata
                    .contract_address
                    .map(|value| alloy_core::primitives::Address::from_str(&value))
                    .transpose()
                    .ok()
                    .flatten();
                (chain_id, contract_address)
            }
        }
    }

    pub fn decode_on_chain_code_from_value_and_type(
        value: &str,
        bytecode_type: BytecodeType,
    ) -> anyhow::Result<OnChainCode> {
        let code_value =
            blockscout_display_bytes::decode_hex(value).context("bytecode is not valid hex")?;
        match bytecode_type {
            BytecodeType::Unspecified => Err(anyhow::anyhow!("bytecode type is unspecified")),
            BytecodeType::CreationInput => Ok(OnChainCode::creation(code_value)),
            BytecodeType::DeployedBytecode => Ok(OnChainCode::runtime(code_value)),
        }
    }

    pub fn decode_compiler_version(value: &str) -> anyhow::Result<DetailedVersion> {
        DetailedVersion::from_str(value).context("invalid compiler version")
    }

    pub fn decode_optional_hex(
        maybe_value: Option<String>,
    ) -> Result<Option<Vec<u8>>, hex::FromHexError> {
        maybe_value
            .map(|value| blockscout_display_bytes::decode_hex(&value))
            .transpose()
    }
}
