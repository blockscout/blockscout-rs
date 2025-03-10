use super::client::Client;
use crate::{compiler::DetailedVersion, BatchError, BatchVerificationResult, Contract};
use bytes::Bytes;
use foundry_compilers::{
    artifacts::{Libraries, Settings, Source, Sources},
    CompilerInput, EvmVersion,
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

pub use multi_part_new::{verify, VerificationRequestNew};
mod multi_part_new {
    use crate::{
        verify_new::{self, SolcInput},
        DetailedVersion, OnChainCode, SolidityClient as Client,
    };
    use alloy_core::primitives::Address;
    use foundry_compilers_new::{
        artifacts,
        artifacts::solc::{BytecodeHash, SettingsMetadata},
    };
    use semver::VersionReq;
    use std::{collections::BTreeMap, ffi::OsStr, path::PathBuf, sync::Arc};

    pub struct VerificationRequestNew {
        pub on_chain_code: OnChainCode,
        pub compiler_version: DetailedVersion,
        pub content: Content,

        // metadata
        pub chain_id: Option<String>,
        pub contract_address: Option<Address>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct Content {
        pub sources: BTreeMap<PathBuf, String>,
        pub evm_version: Option<artifacts::EvmVersion>,
        pub optimization_runs: Option<usize>,
        pub contract_libraries: BTreeMap<String, String>,
    }

    impl From<Content> for Vec<SolcInput> {
        fn from(content: Content) -> Self {
            let mut settings = artifacts::solc::Settings::default();
            if let Some(optimization_runs) = content.optimization_runs {
                settings.optimizer.enabled = Some(true);
                settings.optimizer.runs = Some(optimization_runs);
            }

            // we have to know filename for library, but we don't know,
            // so we assume that every file MAY contain all libraries
            let libs = content
                .sources
                .keys()
                .map(|filename| (PathBuf::from(filename), content.contract_libraries.clone()))
                .collect();
            settings.libraries = artifacts::solc::Libraries { libs };

            settings.evm_version = content.evm_version;

            let sources: artifacts::Sources = content
                .sources
                .into_iter()
                .map(|(name, content)| (name, artifacts::Source::new(content)))
                .collect();
            let inputs: Vec<_> = input_from_sources_and_settings(sources, settings.clone())
                .into_iter()
                .map(SolcInput)
                .collect();
            inputs
        }
    }

    pub async fn verify(
        client: Arc<Client>,
        request: VerificationRequestNew,
    ) -> Result<verify_new::VerificationResult, verify_new::Error> {
        let to_verify = vec![verify_new::OnChainContract {
            on_chain_code: request.on_chain_code,
        }];
        let compilers = client.new_compilers();

        let solc_inputs: Vec<SolcInput> = request.content.into();
        for solc_input in solc_inputs {
            for metadata in settings_metadata(&request.compiler_version) {
                let mut solc_input = solc_input.clone();
                solc_input.0.settings.metadata = metadata;

                let results = verify_new::compile_and_verify(
                    to_verify.clone(),
                    compilers,
                    &request.compiler_version,
                    solc_input,
                )
                .await?;
                let result = results
                    .into_iter()
                    .next()
                    .expect("we sent exactly one contract to verify");

                if result.is_empty() {
                    continue;
                }

                return Ok(result);
            }
        }

        // no contracts could be verified
        Ok(vec![])
    }

    fn input_from_sources_and_settings(
        sources: artifacts::Sources,
        settings: artifacts::solc::Settings,
    ) -> Vec<artifacts::SolcInput> {
        let mut solidity_sources = BTreeMap::new();
        let mut yul_sources = BTreeMap::new();
        for (path, source) in sources {
            if path.extension() == Some(OsStr::new("yul")) {
                yul_sources.insert(path, source);
            } else {
                solidity_sources.insert(path, source);
            }
        }
        let mut res = Vec::new();
        if !yul_sources.is_empty() {
            res.push(artifacts::SolcInput {
                language: artifacts::SolcLanguage::Yul,
                sources: artifacts::Sources(yul_sources),
                settings: settings.clone(),
            });
        }
        if !solidity_sources.is_empty() {
            res.push(artifacts::SolcInput {
                language: artifacts::SolcLanguage::Solidity,
                sources: artifacts::Sources(solidity_sources),
                settings,
            });
        }
        res
    }

    /// Iterates through possible bytecode if required and creates
    /// a corresponding variants of settings metadata for each of them.
    ///
    /// Multi-file input type does not specify it explicitly, thus, we may
    /// have to iterate through all possible options.
    ///
    /// See "settings_metadata" (https://docs.soliditylang.org/en/v0.8.15/using-the-compiler.html?highlight=compiler%20input#input-description)
    fn settings_metadata(compiler_version: &DetailedVersion) -> Vec<Option<SettingsMetadata>> {
        // Options are sorted by their probability of occurring
        const BYTECODE_HASHES: [BytecodeHash; 3] =
            [BytecodeHash::Ipfs, BytecodeHash::None, BytecodeHash::Bzzr1];

        if VersionReq::parse("<0.6.0")
            .unwrap()
            .matches(compiler_version.version())
        {
            [None].into()
        } else {
            BYTECODE_HASHES
                .map(|hash| Some(SettingsMetadata::from(hash)))
                .into()
        }
    }

    mod proto {
        use super::*;
        use crate::solidity::RequestParseError;
        use anyhow::Context;
        use foundry_compilers_new::artifacts::solc::EvmVersion;
        use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
            BytecodeType, VerifySolidityMultiPartRequest,
        };
        use std::str::FromStr;

        impl TryFrom<VerifySolidityMultiPartRequest> for VerificationRequestNew {
            type Error = RequestParseError;

            fn try_from(request: VerifySolidityMultiPartRequest) -> Result<Self, Self::Error> {
                let code_value = blockscout_display_bytes::decode_hex(&request.bytecode)
                    .context("bytecode is not valid hex")?;
                let on_chain_code = match request.bytecode_type() {
                    BytecodeType::Unspecified => {
                        Err(anyhow::anyhow!("bytecode type is unspecified"))?
                    }
                    BytecodeType::CreationInput => OnChainCode::creation(code_value),
                    BytecodeType::DeployedBytecode => OnChainCode::runtime(code_value),
                };

                let compiler_version = DetailedVersion::from_str(&request.compiler_version)
                    .context("invalid compiler version")?;

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

                let (chain_id, contract_address) = match request.metadata {
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
                };

                Ok(Self {
                    on_chain_code,
                    compiler_version,
                    content: Content {
                        sources,
                        evm_version,
                        optimization_runs: request.optimization_runs.map(|i| i as usize),
                        contract_libraries: request.libraries,
                    },
                    chain_id,
                    contract_address,
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: DetailedVersion,

    pub content: MultiFileContent,

    // Required for the metrics. Has no functional meaning.
    // In case if chain_id has not been provided, results in empty string.
    pub chain_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiFileContent {
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: Option<EvmVersion>,
    pub optimization_runs: Option<usize>,
    pub contract_libraries: Option<BTreeMap<String, String>>,
}

impl From<MultiFileContent> for Vec<CompilerInput> {
    fn from(content: MultiFileContent) -> Self {
        let mut settings = Settings::default();
        if let Some(optimization_runs) = content.optimization_runs {
            settings.optimizer.enabled = Some(true);
            settings.optimizer.runs = Some(optimization_runs);
        }

        if let Some(libs) = content.contract_libraries {
            // we have to know filename for library, but we don't know,
            // so we assume that every file MAY contains all libraries
            let libs = content
                .sources
                .keys()
                .map(|filename| (PathBuf::from(filename), libs.clone()))
                .collect();
            settings.libraries = Libraries { libs };
        }
        settings.evm_version = content.evm_version;

        let sources: Sources = content
            .sources
            .into_iter()
            .map(|(name, content)| (name, Source::new(content)))
            .collect();
        let inputs: Vec<_> = input_from_sources(sources)
            .into_iter()
            .map(|input| input.settings(settings.clone()))
            .collect();
        inputs
    }
}

const SOLIDITY: &str = "Solidity";
const YUL: &str = "Yul";

fn input_from_sources(sources: Sources) -> Vec<CompilerInput> {
    let mut solidity_sources = BTreeMap::new();
    let mut yul_sources = BTreeMap::new();
    for (path, source) in sources {
        if path.to_str().unwrap_or_default().ends_with(".yul") {
            yul_sources.insert(path, source);
        } else {
            solidity_sources.insert(path, source);
        }
    }
    let mut res = Vec::new();
    if !solidity_sources.is_empty() {
        res.push(CompilerInput {
            language: SOLIDITY.to_string(),
            sources: solidity_sources,
            settings: Default::default(),
        });
    }
    if !yul_sources.is_empty() {
        res.push(CompilerInput {
            language: YUL.to_string(),
            sources: yul_sources,
            settings: Default::default(),
        });
    }
    res
}

pub struct BatchVerificationRequest {
    pub contracts: Vec<Contract>,
    pub compiler_version: DetailedVersion,
    pub content: MultiFileContent,
}

pub async fn batch_verify(
    client: Arc<Client>,
    request: BatchVerificationRequest,
) -> Result<Vec<BatchVerificationResult>, BatchError> {
    let compiler_inputs: Vec<CompilerInput> = request.content.into();

    if compiler_inputs.len() != 1 {
        return Err(BatchError::Compilation(vec![
            "Either `.sol` or `.yul` files should exist. Not both.".to_string(),
        ]));
    }
    let compiler_input = compiler_inputs.into_iter().next().unwrap();

    let verification_result = crate::batch_verifier::verify_solidity(
        client.compilers(),
        request.compiler_version,
        request.contracts,
        &compiler_input,
    )
    .await?;

    Ok(verification_result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sources(sources: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
        sources
            .iter()
            .map(|(name, content)| (PathBuf::from(name), content.to_string()))
            .collect()
    }

    fn test_to_input(multi_part: MultiFileContent, expected: Vec<&str>) {
        let inputs: Vec<CompilerInput> = multi_part.into();
        assert_eq!(
            inputs.len(),
            expected.len(),
            "invalid number of compiler inputs"
        );
        for i in 0..expected.len() {
            let input_json = serde_json::to_string(&inputs[i]).unwrap();
            println!("{input_json}");
            assert_eq!(input_json, expected[i]);
        }
    }

    #[test]
    fn multi_part_to_input() {
        let multi_part = MultiFileContent {
            sources: sources(&[("source.sol", "pragma")]),
            evm_version: Some(EvmVersion::London),
            optimization_runs: Some(200),
            contract_libraries: Some(BTreeMap::from([(
                "some_library".into(),
                "some_address".into(),
            )])),
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london","libraries":{"source.sol":{"some_library":"some_address"}}}}"#;
        test_to_input(multi_part, vec![expected]);
        let multi_part = MultiFileContent {
            sources: sources(&[("source.sol", "")]),
            evm_version: Some(EvmVersion::SpuriousDragon),
            optimization_runs: None,
            contract_libraries: None,
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":""}},"settings":{"optimizer":{"enabled":false,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"spuriousDragon","libraries":{}}}"#;
        test_to_input(multi_part, vec![expected]);
    }

    #[test]
    fn yul_and_solidity_to_inputs() {
        let multi_part = MultiFileContent {
            sources: sources(&[
                ("source.sol", "pragma"),
                ("source2.yul", "object \"A\" {}"),
                (".yul", "object \"A\" {}"),
            ]),
            evm_version: Some(EvmVersion::London),
            optimization_runs: Some(200),
            contract_libraries: None,
        };
        let expected_solidity = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london","libraries":{}}}"#;
        let expected_yul = r#"{"language":"Yul","sources":{".yul":{"content":"object \"A\" {}"},"source2.yul":{"content":"object \"A\" {}"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["ast"],"*":["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"]}},"evmVersion":"london","libraries":{}}}"#;
        test_to_input(multi_part, vec![expected_solidity, expected_yul]);
    }
}
