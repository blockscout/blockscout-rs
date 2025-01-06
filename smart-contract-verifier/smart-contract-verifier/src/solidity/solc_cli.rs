//! Module for compiling solidity contracts using cmd args.
//! ethers_solc compiler uses --standard-json method of input,
//! because it's very easy and convient, however --standard-json flag
//! was added only since 0.4.10 version. So, to compile older versions
//! we need convert functions for CompilerInput and CompilerOutput.

use ethers_solc::{
    artifacts::Severity,
    error::{SolcError, SolcIoError},
    CompilerOutput,
};
use foundry_compilers::CompilerInput;
use std::{collections::BTreeMap, path::Path, process::Stdio};
use tokio::process::Command;

mod serde_helpers {
    use serde::de;

    pub fn deserialize_abi_string<'de, D>(
        deserializer: D,
    ) -> Result<Vec<serde_json::Value>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s: String = de::Deserialize::deserialize(deserializer)?;
        serde_json::from_str(&s).map_err(de::Error::custom)
    }
}

mod types {
    use super::serde_helpers;
    use ethers_solc::{artifacts::Contract, error::SolcError, CompilerOutput};
    use foundry_compilers::{artifacts::Libraries, CompilerInput};
    use serde::{Deserialize, Serialize};
    use std::{
        collections::{BTreeMap, HashMap},
        path::PathBuf,
    };
    use tempfile::TempDir;
    use tokio::io::AsyncWriteExt;

    #[derive(Debug, PartialEq, Eq)]
    pub struct InputArgs {
        pub optimize: bool,
        pub optimize_runs: Option<usize>,
        pub libs: BTreeMap<String, String>,
    }

    fn merge_libs(libraries: Libraries) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();
        libraries
            .libs
            .into_iter()
            .for_each(|(_, libs)| result.extend(libs));
        result
    }

    impl TryFrom<&CompilerInput> for InputArgs {
        type Error = SolcError;
        fn try_from(input: &CompilerInput) -> Result<Self, Self::Error> {
            let libs = merge_libs(input.settings.libraries.clone());
            Ok(InputArgs {
                optimize: input.settings.optimizer.enabled.unwrap_or(false),
                optimize_runs: input.settings.optimizer.runs,
                libs,
            })
        }
    }

    impl InputArgs {
        pub fn build(&self) -> Vec<String> {
            let mut vec: Vec<String> = vec![
                "--combined-json".to_string(),
                OutputContract::keys().join(","),
            ];
            if self.optimize {
                vec.push("--optimize".to_string());
            }
            if let Some(runs) = self.optimize_runs {
                // Note: --optimize-runs doesn't affect bytecode without --optimize
                vec.push("--optimize-runs".to_string());
                vec.push(runs.to_string());
            }
            if !self.libs.is_empty() {
                vec.push("--libraries".to_string());
                let libs: Vec<String> = self
                    .libs
                    .iter()
                    .map(|(name, address)| format!("{name}:{address}"))
                    .collect();
                vec.push(libs.join(","))
            }
            vec
        }
    }

    #[derive(Debug)]
    pub struct InputFiles {
        pub files_dir: TempDir,
        pub file_names: Vec<PathBuf>,
    }

    impl InputFiles {
        pub async fn try_from_compiler_input(input: &CompilerInput) -> Result<Self, SolcError> {
            if !input.sources.is_empty() {
                let files_dir =
                    tempfile::tempdir().map_err(|e| SolcError::Message(e.to_string()))?;
                let mut file_names = Vec::new();
                for (name, source) in input.sources.iter() {
                    let file_path = files_dir.path().join(name);
                    // name itself may contain some paths inside
                    let prefix = file_path.parent();
                    if let Some(prefix) = prefix {
                        tokio::fs::create_dir_all(prefix)
                            .await
                            .map_err(|e| SolcError::Message(e.to_string()))?;
                    }
                    let mut file = tokio::fs::File::create(&file_path)
                        .await
                        .map_err(|e| SolcError::Message(e.to_string()))?;
                    file_names.push(file_path);
                    file.write_all(source.content.as_bytes())
                        .await
                        .map_err(|e| SolcError::Message(e.to_string()))?;
                }

                Ok(InputFiles {
                    files_dir,
                    file_names,
                })
            } else {
                Err(SolcError::Message("no files were provided".to_string()))
            }
        }

        pub fn build(&self) -> Result<&Vec<PathBuf>, SolcError> {
            self.files_dir
                .path()
                .exists()
                .then_some(&self.file_names)
                .ok_or_else(|| {
                    SolcError::Message("temp dir with contracts doesn't exist".to_string())
                })
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    pub struct OutputContract {
        #[serde(deserialize_with = "serde_helpers::deserialize_abi_string")]
        pub abi: Vec<serde_json::Value>,
        pub bin: String,
        #[serde(rename = "bin-runtime")]
        pub bin_runtime: String,
    }

    impl OutputContract {
        fn keys() -> Vec<String> {
            vec![
                "abi".to_string(),
                "bin".to_string(),
                "bin-runtime".to_string(),
            ]
        }
    }

    impl TryFrom<OutputContract> for Contract {
        type Error = SolcError;

        fn try_from(output_contract: OutputContract) -> Result<Self, Self::Error> {
            let contract = serde_json::json!({
                "abi": output_contract.abi,
                "evm": {
                    "bytecode": {
                        "object": output_contract.bin,
                    },
                    "deployedBytecode": {
                        "object": output_contract.bin_runtime,
                    },
                },
            });
            let contract: Contract = serde_json::from_value(contract)?;
            Ok(contract)
        }
    }

    #[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
    pub struct OutputJson {
        pub contracts: HashMap<String, OutputContract>,
    }

    fn remove_path_from_contract_name(name: String) -> String {
        name.rsplit_once(':')
            .map(|(_, name_cleared)| name_cleared.to_string())
            .unwrap_or(name)
    }

    impl TryFrom<OutputJson> for CompilerOutput {
        type Error = SolcError;

        fn try_from(output_json: OutputJson) -> Result<Self, Self::Error> {
            let contracts = output_json
                .contracts
                .into_iter()
                .map(|(name, output)| {
                    let name = remove_path_from_contract_name(name);
                    output.try_into().map(|contract| (name, contract))
                })
                .collect::<Result<HashMap<String, Contract>, _>>()?;
            let contracts_raw = serde_json::to_value(&contracts)?;
            let compiler_output = serde_json::json!(
                {
                    "contracts": {
                        // TODO: give filename, if only 1 file was provided
                        "": contracts_raw
                    }
                }
            );
            let compiler_output = serde_json::from_value(compiler_output)?;
            Ok(compiler_output)
        }
    }
}

fn compiler_error(message: String) -> ethers_solc::artifacts::Error {
    ethers_solc::artifacts::Error {
        source_location: None,
        secondary_source_locations: vec![],
        r#type: "".to_string(),
        component: "".to_string(),
        severity: Severity::Error,
        error_code: None,
        message,
        formatted_message: None,
    }
}

pub async fn compile_using_cli(
    solc: &Path,
    input: &CompilerInput,
) -> Result<CompilerOutput, SolcError> {
    let output = {
        let input_args = types::InputArgs::try_from(input)?;
        let input_files = types::InputFiles::try_from_compiler_input(input).await?;
        Command::new(solc)
            .args(input_args.build())
            .args(input_files.build()?)
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|err| SolcError::Io(SolcIoError::new(err, solc)))?
    };

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let compiler_output = if output.stderr.is_empty() {
        let output_json: types::OutputJson = serde_json::from_slice(output.stdout.as_slice())?;
        CompilerOutput::try_from(output_json)?
    } else {
        CompilerOutput {
            errors: vec![compiler_error(stderr)],
            sources: BTreeMap::new(),
            contracts: BTreeMap::new(),
        }
    };
    Ok(compiler_output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{DetailedVersion, Fetcher, ListFetcher};
    use ethers_solc::Artifact;
    use foundry_compilers::artifacts::{Settings, Source};
    use hex::ToHex;
    use pretty_assertions::assert_eq;
    use std::{collections::HashSet, env::temp_dir, path::PathBuf, str::FromStr};
    use url::Url;
    use wiremock::{
        matchers::{method, path},
        Mock, MockServer, ResponseTemplate,
    };

    const DEFAULT_COMPILER_INPUT: &str = r#"
    {
        "language": "Solidity",
        "sources": {
            "a.sol": {
            "content": "pragma solidity >=0.4.5;\n\n\ncontract A {\n   function get_a() public returns (uint256) {\n        return 88888888888;\n    }\n}"
            },
            "b.sol": {
            "content": "pragma solidity >=0.4.5;\n\n\ncontract B {\n   function get_b() public returns (uint256) {\n        return 7777777777777;\n    }\n}"
            },
            "main.sol": {
            "content": "pragma solidity >=0.4.5;\n\nimport \"./a.sol\";\n\nimport \"./b.sol\";\n\ncontract Main is A, B {\n   function get() public returns (uint256) {\n        return get_a() + get_b();\n    }\n}\n\n"
            }
        },
        "settings": {
            "optimizer": {
                "enabled": false,
                "runs": 200
            },
            "libraries": {
                "main.sol": {
                    "MyLib": "0x1234567890123456789012345678901234567890",
                    "OurLib": "0x0987654321098765432109876543210987654321"
                }
            },
            "outputSelection": {
                "*": {
                "*": ["*"]
                }
            }
            }
    }
    "#;

    const DEFAULT_COMPILER_OUTPUT: &str = r#"
    {
        "contracts": {
            "A": {
                "abi": "[{\"constant\":false,\"inputs\":[],\"name\":\"get_a\",\"outputs\":[{\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"type\":\"function\"}]",
                "bin": "6060604052346000575b6096806100176000396000f30060606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680634815918114603c575b6000565b346000576046605c565b6040518082815260200191505060405180910390f35b60006414b230ce3890505b905600a165627a7a7230582062ac15c74e3af0aec92b47f64d9c8909939b731732d5ee4163c6ed3af70806550029",
                "bin-runtime": "60606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff1680634815918114603c575b6000565b346000576046605c565b6040518082815260200191505060405180910390f35b60006414b230ce3890505b905600a165627a7a7230582062ac15c74e3af0aec92b47f64d9c8909939b731732d5ee4163c6ed3af70806550029"
            },
            "B": {
                "abi": "[{\"constant\":false,\"inputs\":[],\"name\":\"get_b\",\"outputs\":[{\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"type\":\"function\"}]",
                "bin": "6060604052346000575b6097806100176000396000f30060606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063f3d33cf414603c575b6000565b346000576046605c565b6040518082815260200191505060405180910390f35b6000650712e7ae7c7190505b905600a165627a7a723058201d98c5b92f01dbead603c6c3b4c7f04520fa048e1eacf0ce2dad63a406019c710029",
                "bin-runtime": "60606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063f3d33cf414603c575b6000565b346000576046605c565b6040518082815260200191505060405180910390f35b6000650712e7ae7c7190505b905600a165627a7a723058201d98c5b92f01dbead603c6c3b4c7f04520fa048e1eacf0ce2dad63a406019c710029"
            },
            "Main": {
                "abi": "[{\"constant\":false,\"inputs\":[],\"name\":\"get_a\",\"outputs\":[{\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"type\":\"function\"},{\"constant\":false,\"inputs\":[],\"name\":\"get\",\"outputs\":[{\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"type\":\"function\"},{\"constant\":false,\"inputs\":[],\"name\":\"get_b\",\"outputs\":[{\"name\":\"\",\"type\":\"uint256\"}],\"payable\":false,\"type\":\"function\"}]",
                "bin": "606060405234610000575b61010e806100196000396000f30060606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063481591811460505780636d4ce63c146070578063f3d33cf4146090575b6000565b34600057605a60b0565b6040518082815260200191505060405180910390f35b34600057607a60be565b6040518082815260200191505060405180910390f35b34600057609a60d3565b6040518082815260200191505060405180910390f35b60006414b230ce3890505b90565b600060c660d3565b60cc60b0565b0190505b90565b6000650712e7ae7c7190505b905600a165627a7a72305820a80a9599b36625e94a3eadfd5c31475a2c507be6d1a9fa9a35e9cd4c54bce1390029",
                "bin-runtime": "60606040526000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff168063481591811460505780636d4ce63c146070578063f3d33cf4146090575b6000565b34600057605a60b0565b6040518082815260200191505060405180910390f35b34600057607a60be565b6040518082815260200191505060405180910390f35b34600057609a60d3565b6040518082815260200191505060405180910390f35b60006414b230ce3890505b90565b600060c660d3565b60cc60b0565b0190505b90565b6000650712e7ae7c7190505b905600a165627a7a72305820a80a9599b36625e94a3eadfd5c31475a2c507be6d1a9fa9a35e9cd4c54bce1390029"
            }
        },
        "version": "0.4.8+commit.60cc1668"
    }
    "#;

    #[cfg(target_os = "linux")]
    pub const LIST_JSON: &str = r#"
    {
        "builds": [
            {
                "path": "https://github.com/blockscout/solc-bin/releases/download/v0.4.8%2Bcommit.60cc1668/solc",
                "longVersion": "v0.4.8+commit.60cc1668",
                "sha256": "9c64d0ea8373346342f462d0ee5a5a50f1946e209e971f8339af5722c5d65144"
            },
            {
                "path": "https://github.com/blockscout/solc-bin/releases/download/v0.4.10%2Bcommit.f0d539ae/solc",
                "longVersion": "v0.4.10+commit.f0d539ae",
                "sha256": "e1897b1985e5091555d97178bf4bd48e85b56d617561d0d5928414e4f007195b"
            }
        ]
    }
    "#;
    #[cfg(target_os = "macos")]
    pub const LIST_JSON: &str = r#"
        {
            "builds": [
                {
                    "path": "https://solc-bin.ethereum.org/macosx-amd64/solc-macosx-amd64-v0.4.8+commit.60cc1668",
                    "longVersion": "v0.4.8+commit.60cc1668",
                    "sha256": "ebb64b8b8dd465bd53a52fa7063569115df176c7561ac4feb47004513e1df74b"
                },
                {
                    "path": "https://solc-bin.ethereum.org/macosx-amd64/solc-macosx-amd64-v0.4.10+commit.f0d539ae",
                    "longVersion": "v0.4.10+commit.f0d539ae",
                    "sha256": "0x40f179e4d27201ab726669dd26d594cfe10bf4dd6117495ee49d26f0dda9ef42"
                }
            ]
        }
    "#;

    #[test]
    fn correct_input_args() {
        let input: CompilerInput = serde_json::from_str(DEFAULT_COMPILER_INPUT).unwrap();

        let input_args = types::InputArgs::try_from(&input).expect("failed to convert args");
        let expected_args = types::InputArgs {
            optimize: false,
            optimize_runs: Some(200),
            libs: BTreeMap::from_iter([
                (
                    "MyLib".to_string(),
                    "0x1234567890123456789012345678901234567890".to_string(),
                ),
                (
                    "OurLib".to_string(),
                    "0x0987654321098765432109876543210987654321".to_string(),
                ),
            ]),
        };
        assert_eq!(input_args, expected_args);
        assert_eq!(
            input_args.build(),
            vec![
                "--combined-json",
                "abi,bin,bin-runtime",
                "--optimize-runs",
                "200",
                "--libraries",
                "MyLib:0x1234567890123456789012345678901234567890,OurLib:0x0987654321098765432109876543210987654321"
            ]
        );
    }

    #[tokio::test]
    async fn correct_input_files() {
        let input: CompilerInput = serde_json::from_str(DEFAULT_COMPILER_INPUT).unwrap();

        let input_files = types::InputFiles::try_from_compiler_input(&input)
            .await
            .expect("failed to convert files");
        assert!(input_files.files_dir.path().exists());

        let expected_files: Vec<PathBuf> = vec!["a.sol", "b.sol", "main.sol"]
            .into_iter()
            .map(|name| input_files.files_dir.path().join(name))
            .collect();
        assert_eq!(input_files.file_names, expected_files);
        let string_args = input_files.build().expect("failed to build string args");
        assert_eq!(string_args, &expected_files);
    }

    #[test]
    fn correct_output() {
        let output_json: types::OutputJson = serde_json::from_str(DEFAULT_COMPILER_OUTPUT).unwrap();
        let compiler_output =
            CompilerOutput::try_from(output_json.clone()).expect("failed to convert output json");
        assert_eq!(compiler_output.contracts.len(), 1);
        let filename = compiler_output.contracts.iter().next().unwrap().0;
        assert_eq!(filename, "");

        for (name, contract) in compiler_output.contracts_iter() {
            let initial_contract = output_json
                .contracts
                .get(name)
                .expect("invalid contract name");
            let abi = serde_json::to_value(&contract.abi).unwrap();
            let expected_abi = serde_json::to_value(&initial_contract.abi).unwrap();
            assert_eq!(abi, expected_abi);

            for (name, actual_bytecode, expected_bytecode) in [
                (
                    "creation bytecode",
                    contract.get_bytecode_bytes(),
                    &initial_contract.bin,
                ),
                (
                    "deployed bytecode",
                    contract.get_deployed_bytecode_bytes(),
                    &initial_contract.bin_runtime,
                ),
            ] {
                let bytecode: String = actual_bytecode
                    .unwrap_or_else(|| panic!("unlinked {name}"))
                    .encode_hex();
                assert_eq!(&bytecode, expected_bytecode, "wrong {}", name);
            }
        }
    }

    async fn get_solc(ver: &DetailedVersion) -> PathBuf {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(LIST_JSON))
            .mount(&mock_server)
            .await;
        let fetcher = ListFetcher::new(
            Url::parse(&mock_server.uri()).unwrap(),
            temp_dir(),
            None,
            None,
        )
        .await
        .expect("failed to build fetcher");
        fetcher
            .fetch(ver)
            .await
            .expect("cannot fetch 0.4.8 version")
    }

    fn source(file: &str, content: &str) -> (PathBuf, Source) {
        (file.into(), Source::new(content))
    }

    #[tokio::test]
    async fn compile() {
        for ver in &["v0.4.8+commit.60cc1668", "v0.4.10+commit.f0d539ae"] {
            let version = DetailedVersion::from_str(ver).expect("valid version");
            let solc = get_solc(&version).await;

            let input: CompilerInput = serde_json::from_str(DEFAULT_COMPILER_INPUT).unwrap();
            let output: CompilerOutput = compile_using_cli(&solc, &input)
                .await
                .unwrap_or_else(|_| panic!("failed to compile contracts with {ver}"));
            assert!(
                !output.has_error(),
                "errors during compilation: {:?}",
                output.errors
            );
            let names: HashSet<String> =
                output.contracts_into_iter().map(|(name, _)| name).collect();
            let expected_names = HashSet::from_iter(["Main".into(), "A".into(), "B".into()]);
            assert_eq!(names, expected_names);

            for sources in [
                BTreeMap::from_iter([source("main.sol", "")]),
                BTreeMap::from_iter([source("main.sol", "some string")]),
            ] {
                let input = CompilerInput {
                    language: "Solidity".into(),
                    sources,
                    settings: Settings::default(),
                };
                let output: CompilerOutput = compile_using_cli(&solc, &input)
                    .await
                    .expect("shouldn't return Err, but Ok with errors field");
                assert!(output.has_error());
            }

            let input = CompilerInput {
                language: "Solidity".into(),
                sources: BTreeMap::new(),
                settings: Settings::default(),
            };
            compile_using_cli(&solc, &input)
                .await
                .expect_err("should not compile empty files");
        }
    }
}
