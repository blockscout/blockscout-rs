use super::{client::Client, types::Success};
use crate::{
    compiler::Version,
    verifier::{ContractVerifier, Error},
};
use bytes::Bytes;
use ethers_solc::{
    artifacts::{
        output_selection::OutputSelection, BytecodeHash, Libraries, Settings, SettingsMetadata,
        Source, Sources,
    },
    CompilerInput, EvmVersion,
};
use semver::VersionReq;
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: Bytes,
    pub creation_bytecode: Option<Bytes>,
    pub compiler_version: Version,

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
        settings.optimizer.enabled = Some(content.optimization_runs.is_some());
        settings.optimizer.runs = content.optimization_runs;

        settings.output_selection = OutputSelection::complete_output_selection();

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

pub async fn verify(client: Arc<Client>, request: VerificationRequest) -> Result<Success, Error> {
    let compiler_version = request.compiler_version;

    let verifier = ContractVerifier::new(
        client.compilers(),
        &compiler_version,
        request.creation_bytecode,
        request.deployed_bytecode,
        request.chain_id,
    )?;

    let compiler_inputs: Vec<CompilerInput> = request.content.into();
    for mut compiler_input in compiler_inputs {
        for metadata in settings_metadata(&compiler_version) {
            compiler_input.settings.metadata = metadata;
            let result = verifier.verify(&compiler_input).await;

            // If no matching contracts have been found, try the next settings metadata option
            if let Err(Error::NoMatchingContracts) = result {
                continue;
            }

            // If any error, it is uncorrectable and should be returned immediately, otherwise
            // we allow middlewares to process success and only then return it to the caller
            let success = Success::from((compiler_input, result?));
            if let Some(middleware) = client.middleware() {
                middleware.call(&success).await;
            }

            return Ok(success);
        }
    }

    // No contracts could be verified
    Err(Error::NoMatchingContracts)
}

/// Iterates through possible bytecode if required and creates
/// a corresponding variants of settings metadata for each of them.
///
/// Multi-file input type does not specify it explicitly, thus, we may
/// have to iterate through all possible options.
///
/// See "settings_metadata" (https://docs.soliditylang.org/en/v0.8.15/using-the-compiler.html?highlight=compiler%20input#input-description)
fn settings_metadata(compiler_version: &Version) -> Vec<Option<SettingsMetadata>> {
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
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["*"],"*":["*"]}},"evmVersion":"london","libraries":{"source.sol":{"some_library":"some_address"}}}}"#;
        test_to_input(multi_part, vec![expected]);
        let multi_part = MultiFileContent {
            sources: sources(&[("source.sol", "")]),
            evm_version: Some(EvmVersion::SpuriousDragon),
            optimization_runs: None,
            contract_libraries: None,
        };
        let expected = r#"{"language":"Solidity","sources":{"source.sol":{"content":""}},"settings":{"optimizer":{"enabled":false},"outputSelection":{"*":{"":["*"],"*":["*"]}},"evmVersion":"spuriousDragon","libraries":{}}}"#;
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
        let expected_solidity = r#"{"language":"Solidity","sources":{"source.sol":{"content":"pragma"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["*"],"*":["*"]}},"evmVersion":"london","libraries":{}}}"#;
        let expected_yul = r#"{"language":"Yul","sources":{".yul":{"content":"object \"A\" {}"},"source2.yul":{"content":"object \"A\" {}"}},"settings":{"optimizer":{"enabled":true,"runs":200},"outputSelection":{"*":{"":["*"],"*":["*"]}},"evmVersion":"london","libraries":{}}}"#;
        test_to_input(multi_part, vec![expected_solidity, expected_yul]);
    }
}

mod proto {
    use super::{MultiFileContent, VerificationRequest};
    use crate::Version;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use conversion_primitives::InvalidArgument;
    use ethers_solc::EvmVersion;
    use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::{
        BytecodeType, VerifySolidityMultiPartRequest,
    };
    use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

    impl TryFrom<VerifySolidityMultiPartRequest> for VerificationRequest {
        type Error = InvalidArgument;

        fn try_from(request: VerifySolidityMultiPartRequest) -> Result<Self, Self::Error> {
            let bytecode = DisplayBytes::from_str(&request.bytecode)
                .map_err(|err| InvalidArgument::new(format!("Invalid bytecode: {err:?}")))?
                .0;
            let (creation_bytecode, deployed_bytecode) = match request.bytecode_type() {
                BytecodeType::Unspecified => {
                    Err(InvalidArgument::new("bytecode type is unspecified"))?
                }
                BytecodeType::CreationInput => (Some(bytecode), bytes::Bytes::new()),
                BytecodeType::DeployedBytecode => (None, bytecode),
            };

            let compiler_version = Version::from_str(&request.compiler_version)
                .map_err(|err| InvalidArgument::new(format!("Invalid compiler version: {err}")))?;

            let sources: BTreeMap<PathBuf, String> = request
                .source_files
                .into_iter()
                .map(|(name, content)| {
                    (
                        PathBuf::from_str(&name).unwrap(), /* TODO: why unwrap? */
                        content,
                    )
                })
                .collect();

            let evm_version = match request.evm_version {
                Some(version) if version != "default" => {
                    Some(EvmVersion::from_str(&version).map_err(InvalidArgument::new)?)
                }
                _ => None,
            };

            Ok(Self {
                deployed_bytecode,
                creation_bytecode,
                compiler_version,
                content: MultiFileContent {
                    sources,
                    evm_version,
                    optimization_runs: request.optimization_runs.map(|i| i as usize),
                    contract_libraries: Some(request.libraries.into_iter().collect()),
                },
                chain_id: request.metadata.and_then(|metadata| metadata.chain_id),
            })
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use pretty_assertions::assert_eq;
        use smart_contract_verifier_proto::blockscout::smart_contract_verifier::v2::VerificationMetadata;

        #[test]
        fn try_into_verification_request() {
            /********** Creation Input **********/

            let mut request = VerifySolidityMultiPartRequest {
                bytecode: "0x1234".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                source_files: BTreeMap::from([("source_path".into(), "source_content".into())]),
                evm_version: Some("london".to_string()),
                optimization_runs: Some(200),
                libraries: BTreeMap::from([("Lib".into(), "0xcafe".into())]),
                metadata: Some(VerificationMetadata {
                    chain_id: Some("1".into()),
                    contract_address: Some("0xcafecafecafecafecafecafecafecafecafecafe".into()),
                }),
                post_actions: vec![],
            };

            let mut expected = VerificationRequest {
                creation_bytecode: Some(DisplayBytes::from_str("0x1234").unwrap().0),
                deployed_bytecode: DisplayBytes::from_str("").unwrap().0,
                compiler_version: Version::from_str("v0.8.17+commit.8df45f5f").unwrap(),
                content: MultiFileContent {
                    sources: BTreeMap::from([("source_path".into(), "source_content".into())]),
                    evm_version: Some(EvmVersion::London),
                    optimization_runs: Some(200),
                    contract_libraries: Some(BTreeMap::from([("Lib".into(), "0xcafe".into())])),
                },
                chain_id: Some("1".into()),
            };

            let verification_request: VerificationRequest = request
                .clone()
                .try_into()
                .expect("Creation input: try_into verification request failed");
            assert_eq!(expected, verification_request, "Creation input");

            /********** Deployed Bytecode **********/

            request.bytecode_type = BytecodeType::DeployedBytecode.into();
            expected.deployed_bytecode = expected.creation_bytecode.take().unwrap();

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Deployed bytecode: try_into verification request failed");
            assert_eq!(expected, verification_request, "Deployed bytecode");
        }

        #[test]
        // 'default' should result in None in MultiFileContent
        fn default_evm_version() {
            let request = VerifySolidityMultiPartRequest {
                bytecode: "".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                source_files: Default::default(),
                evm_version: Some("default".to_string()),
                optimization_runs: None,
                libraries: Default::default(),
                metadata: None,
                post_actions: vec![],
            };

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Try_into verification request failed");

            assert_eq!(
                None, verification_request.content.evm_version,
                "'default' should result in `None`"
            )
        }

        #[test]
        // 'null' should result in None in MultiFileContent
        fn null_evm_version() {
            let request = VerifySolidityMultiPartRequest {
                bytecode: "".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                source_files: Default::default(),
                evm_version: None,
                optimization_runs: None,
                libraries: Default::default(),
                metadata: None,
                post_actions: vec![],
            };

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Try_into verification request failed");

            assert_eq!(
                None, verification_request.content.evm_version,
                "Absent evm_version should result in `None`"
            )
        }

        #[test]
        fn empty_metadata() {
            let request = VerifySolidityMultiPartRequest {
                bytecode: "".to_string(),
                bytecode_type: BytecodeType::CreationInput.into(),
                compiler_version: "v0.8.17+commit.8df45f5f".to_string(),
                source_files: Default::default(),
                evm_version: None,
                optimization_runs: None,
                libraries: Default::default(),
                metadata: None,
                post_actions: vec![],
            };

            let verification_request: VerificationRequest = request
                .try_into()
                .expect("Try_into verification request failed");

            assert_eq!(
                None, verification_request.chain_id,
                "Absent verification metadata should result in chain_id=None"
            )
        }
    }
}
