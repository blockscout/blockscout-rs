pub mod solidity {
    use bytes::Bytes;
    use foundry_compilers::{CompilerInput, EvmVersion};
    use smart_contract_verifier::{
        solidity::{multi_part, standard_json},
        DetailedVersion as CompilerVersion,
    };
    use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

    pub struct VerificationRequest {
        deployed_bytecode: Bytes,
        creation_bytecode: Option<Bytes>,
        compiler_version: CompilerVersion,
        sources: BTreeMap<PathBuf, String>,
        evm_version: Option<EvmVersion>,
        optimization_runs: Option<usize>,
        contract_libraries: Option<BTreeMap<String, String>>,
    }

    impl VerificationRequest {
        pub fn new(
            deployed_bytecode: &str,
            creation_bytecode: &str,
            compiler_version: &str,
            sources: BTreeMap<String, String>,
            evm_version: Option<String>,
            optimization_runs: Option<usize>,
            contract_libraries: Option<BTreeMap<String, String>>,
        ) -> Result<Self, anyhow::Error> {
            Ok(Self {
                deployed_bytecode: blockscout_display_bytes::Bytes::from_str(deployed_bytecode)
                    .map_err(anyhow::Error::new)?
                    .0,
                creation_bytecode: Some(
                    blockscout_display_bytes::Bytes::from_str(creation_bytecode)
                        .map_err(anyhow::Error::new)?
                        .0,
                ),
                compiler_version: CompilerVersion::from_str(compiler_version)
                    .map_err(anyhow::Error::new)?,
                sources: sources
                    .into_iter()
                    .map(|(k, v)| (PathBuf::from_str(&k).unwrap(), v))
                    .collect::<BTreeMap<_, _>>(),
                evm_version: match evm_version {
                    None => None,
                    Some(version) => {
                        Some(EvmVersion::from_str(&version).map_err(|err| anyhow::anyhow!(err))?)
                    }
                },
                optimization_runs,
                contract_libraries,
            })
        }
    }

    impl From<VerificationRequest> for multi_part::VerificationRequest {
        fn from(source: VerificationRequest) -> Self {
            Self {
                deployed_bytecode: source.deployed_bytecode,
                creation_bytecode: source.creation_bytecode,
                compiler_version: source.compiler_version,
                content: multi_part::MultiFileContent {
                    sources: source.sources,
                    evm_version: source.evm_version,
                    optimization_runs: source.optimization_runs,
                    contract_libraries: source.contract_libraries,
                },
                chain_id: Default::default(),
            }
        }
    }

    impl From<VerificationRequest> for standard_json::VerificationRequest {
        fn from(source: VerificationRequest) -> Self {
            let multi_part_request = multi_part::VerificationRequest::from(source);
            let input = {
                let input: Vec<CompilerInput> = multi_part_request.content.into();
                input.into_iter().next().expect(
                    "Invalid input length when converting into standard_json::VerificationRequest",
                )
            };

            Self {
                deployed_bytecode: multi_part_request.deployed_bytecode,
                creation_bytecode: multi_part_request.creation_bytecode,
                compiler_version: multi_part_request.compiler_version,
                content: standard_json::StandardJsonContent { input },
                chain_id: Default::default(),
            }
        }
    }
}

pub mod vyper {
    use bytes::Bytes;
    use foundry_compilers::EvmVersion;
    use smart_contract_verifier::{vyper::multi_part, DetailedVersion as CompilerVersion};
    use std::{collections::BTreeMap, path::PathBuf, str::FromStr};

    pub struct VerificationRequest {
        deployed_bytecode: Bytes,
        creation_bytecode: Option<Bytes>,
        compiler_version: CompilerVersion,
        sources: BTreeMap<PathBuf, String>,
        evm_version: Option<EvmVersion>,
    }

    impl VerificationRequest {
        pub fn new(
            deployed_bytecode: &str,
            creation_bytecode: &str,
            compiler_version: &str,
            sources: BTreeMap<String, String>,
            evm_version: Option<String>,
        ) -> Result<Self, anyhow::Error> {
            Ok(Self {
                deployed_bytecode: blockscout_display_bytes::Bytes::from_str(deployed_bytecode)
                    .map_err(anyhow::Error::new)?
                    .0,
                creation_bytecode: Some(
                    blockscout_display_bytes::Bytes::from_str(creation_bytecode)
                        .map_err(anyhow::Error::new)?
                        .0,
                ),
                compiler_version: CompilerVersion::from_str(compiler_version)
                    .map_err(anyhow::Error::new)?,
                sources: sources
                    .into_iter()
                    .map(|(k, v)| (PathBuf::from_str(&k).unwrap(), v))
                    .collect::<BTreeMap<_, _>>(),
                evm_version: match evm_version {
                    None => None,
                    Some(version) => {
                        Some(EvmVersion::from_str(&version).map_err(|err| anyhow::anyhow!(err))?)
                    }
                },
            })
        }
    }

    impl From<VerificationRequest> for multi_part::VerificationRequest {
        fn from(source: VerificationRequest) -> Self {
            Self {
                deployed_bytecode: source.deployed_bytecode,
                creation_bytecode: source.creation_bytecode,
                compiler_version: source.compiler_version,
                content: multi_part::MultiFileContent {
                    sources: source.sources,
                    interfaces: Default::default(),
                    evm_version: source.evm_version,
                },
                chain_id: Default::default(),
            }
        }
    }
}
