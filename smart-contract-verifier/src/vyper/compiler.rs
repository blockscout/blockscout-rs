use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Stdio, Command};
use std::str::FromStr;
use ethers_solc::artifacts::{Ast, Contract, NodeType, Severity, output_selection::OutputSelection};
use serde::{Deserialize, Serialize};
use colored::Colorize;

use super::errors::VyperError;

pub type Sources = BTreeMap<PathBuf, Source>;
pub type FileToContractsMap<T> = BTreeMap<String, BTreeMap<String, T>>;
/// file -> (contract name -> Contract)
pub type Contracts = FileToContractsMap<Contract>;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct Source {
    pub content: String,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvmVersion {
    Byzantium,
    Constantinople,
    Petersburg,
    Istanbul,
}

impl Default for EvmVersion {
    fn default() -> Self {
        Self::Istanbul
    }
}

// impl EvmVersion {
//     /// Checks against the given solidity `semver::Version`
//     pub fn normalize_version(self, version: &Version) -> Option<EvmVersion> {
//         // the EVM version flag was only added at 0.4.21
//         // we work our way backwards
//         if version >= &CONSTANTINOPLE_SOLC {
//             // If the Solc is at least at london, it supports all EVM versions
//             Some(if version >= &LONDON_SOLC {
//                 self
//                 // For all other cases, cap at the at-the-time highest possible
//                 // fork
//             } else if version >= &BERLIN_SOLC && self >= EvmVersion::Berlin {
//                 EvmVersion::Berlin
//             } else if version >= &ISTANBUL_SOLC && self >= EvmVersion::Istanbul {
//                 EvmVersion::Istanbul
//             } else if version >= &PETERSBURG_SOLC && self >= EvmVersion::Petersburg {
//                 EvmVersion::Petersburg
//             } else if self >= EvmVersion::Constantinople {
//                 EvmVersion::Constantinople
//             } else {
//                 self
//             })
//         } else {
//             None
//         }
//     }
// }

impl fmt::Display for EvmVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let string = match self {
            EvmVersion::Byzantium => "byzantium",
            EvmVersion::Constantinople => "constantinople",
            EvmVersion::Petersburg => "petersburg",
            EvmVersion::Istanbul => "istanbul",
        };
        write!(f, "{}", string)
    }
}

impl FromStr for EvmVersion {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "byzantium" => Ok(EvmVersion::Byzantium),
            "constantinople" => Ok(EvmVersion::Constantinople),
            "petersburg" => Ok(EvmVersion::Petersburg),
            "istanbul" => Ok(EvmVersion::Istanbul),
            s => Err(format!("Unknown evm version: {}", s)),
        }
    }
}

// #[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
// #[serde(transparent)]
// pub struct OutputSelection(pub BTreeMap<String, Vec<String>>);
//
// impl OutputSelection {
//     /// Select all possible compiler outputs: "outputSelection: { "*": ["*"] }"
//     /// Note that this might slow down the compilation process needlessly.
//     pub fn complete_output_selection() -> Self {
//         BTreeMap::from([(
//             "*".to_string(),
//             vec!["*".to_string()]
//         )])
//             .into()
//     }
//
//     /// Default output selection for compiler output:
//     ///
//     /// `{ "*": ["abi","evm.bytecode","evm.deployedBytecode","evm.methodIdentifiers"] }`
//     pub fn default_output_selection() -> Self {
//         BTreeMap::from([(
//             "*".to_string(),
//             vec![
//                 "abi".to_string(),
//                 "evm.bytecode".to_string(),
//                 "evm.deployedBytecode".to_string(),
//                 "evm.methodIdentifiers".to_string(),
//             ],
//         )]).into()
//     }
// }
//
// /// TODO: test empty array
//
// impl AsRef<BTreeMap<String, Vec<String>>> for OutputSelection {
//     fn as_ref(&self) -> &BTreeMap<String, Vec<String>> {
//         &self.0
//     }
// }
//
// impl AsMut<BTreeMap<String, Vec<String>>> for OutputSelection {
//     fn as_mut(&mut self) -> &mut BTreeMap<String, Vec<String>> {
//         &mut self.0
//     }
// }
//
// impl From<BTreeMap<String, Vec<String>>> for OutputSelection {
//     fn from(s: BTreeMap<String, Vec<String>>) -> Self {
//         OutputSelection(s)
//     }
// }

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    #[serde(
    default,
    skip_serializing_if = "Option::is_none")]
    pub evm_version: Option<EvmVersion>,
    pub optimize: bool,
    #[serde(default)]
    pub output_selection: OutputSelection,
}

impl Settings {
    /// Creates a new `Settings` instance with the given `output_selection`
    pub fn new(output_selection: impl Into<OutputSelection>) -> Self {
        Self { output_selection: output_selection.into(), ..Default::default() }
    }

    // /// Inserts a set of `ContractOutputSelection`
    // pub fn push_all(&mut self, settings: impl IntoIterator<Item = ContractOutputSelection>) {
    //     for value in settings {
    //         self.push_output_selection(value)
    //     }
    // }
    //
    // /// Inserts a set of `ContractOutputSelection`
    // #[must_use]
    // pub fn with_extra_output(
    //     mut self,
    //     settings: impl IntoIterator<Item = ContractOutputSelection>,
    // ) -> Self {
    //     for value in settings {
    //         self.push_output_selection(value)
    //     }
    //     self
    // }
    //
    // /// Inserts the value for all files and contracts
    // ///
    // /// ```
    // /// use ethers_solc::artifacts::output_selection::ContractOutputSelection;
    // /// use ethers_solc::artifacts::Settings;
    // /// let mut selection = Settings::default();
    // /// selection.push_output_selection(ContractOutputSelection::Metadata);
    // /// ```
    // pub fn push_output_selection(&mut self, value: impl ToString) {
    //     self.push_contract_output_selection("*", value)
    // }

    /// Inserts the `key` `value` pair to the `output_selection` for all files
    ///
    /// If the `key` already exists, then the value is added to the existing list
    pub fn push_contract_output_selection(
        &mut self,
        contracts: impl Into<String>,
        value: impl ToString,
    ) {
        let value = value.to_string();
        let values = self
            .output_selection
            .as_mut()
            .entry("*".to_string())
            .or_default()
            .entry(contracts.into())
            .or_default();
        if !values.contains(&value) {
            values.push(value)
        }
    }

    /// Sets the value for all files and contracts
    pub fn set_output_selection(&mut self, values: impl IntoIterator<Item = impl ToString>) {
        self.set_contract_output_selection("*", values)
    }

    /// Sets the `key` to the `values` pair to the `output_selection` for all files
    ///
    /// This will replace the existing values for `key` if they're present
    pub fn set_contract_output_selection(
        &mut self,
        key: impl Into<String>,
        values: impl IntoIterator<Item = impl ToString>,
    ) {
        self.output_selection
            .as_mut()
            .entry("*".to_string())
            .or_default()
            .insert(key.into(), values.into_iter().map(|s| s.to_string()).collect());
    }

    /// Adds `ast` to output
    #[must_use]
    pub fn with_ast(mut self) -> Self {
        let output =
            self.output_selection.as_mut().entry("*".to_string()).or_insert_with(BTreeMap::default);
        output.insert("".to_string(), vec!["ast".to_string()]);
        self
    }

    /// Strips `base` from all paths
    pub fn with_base_path(mut self, base: impl AsRef<Path>) -> Self {
        let base = base.as_ref();

        self.output_selection = OutputSelection(
            self.output_selection
                .0
                .into_iter()
                .map(|(file, selection)| {
                    (
                        Path::new(&file)
                            .strip_prefix(base)
                            .map(|p| format!("{}", p.display()))
                            .unwrap_or(file),
                        selection,
                    )
                })
                .collect(),
        );
        self
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            optimize: true,
            output_selection: OutputSelection::default_output_selection(),
            evm_version: Some(EvmVersion::default()),
        }
            .with_ast()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompilerInput {
    pub language: String,
    pub sources: Sources,
    // pub interfaces: ,
    pub settings: Settings,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<SourceLocation>,
    pub r#type: String,
    pub component: String,
    pub severity: Severity,
    pub message: String,
    pub formatted_message: Option<String>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(msg) = &self.formatted_message {
            match self.severity {
                Severity::Error => {
                    msg.as_str().red().fmt(f)
                }
                Severity::Warning | Severity::Info => {
                    msg.as_str().yellow().fmt(f)
                }
            }
        } else {
            self.severity.fmt(f)?;
            writeln!(f, ": {}", self.message)
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct SourceLocation {
    pub file: String,
    // pub lineno: i32,
    // pub col_offset: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
pub struct SourceFile {
    pub id: u32,
    // #[serde(default)]
    // pub ast: Option<Ast>,
}
//
// impl SourceFile {
//     /// Returns `true` if the source file contains at least 1 `ContractDefinition` such as
//     /// `contract`, `abstract contract`, `interface` or `library`
//     pub fn contains_contract_definition(&self) -> bool {
//         if let Some(ref ast) = self.ast {
//             // contract definitions are only allowed at the source-unit level <https://docs.soliditylang.org/en/latest/grammar.html>
//             return ast.nodes.iter().any(|node| node.node_type == NodeType::ContractDefinition)
//             // abstract contract, interfaces: ContractDefinition
//         }
//
//         false
//     }
// }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
pub struct CompilerOutput {
    pub compiler: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<Error>,
    #[serde(default)]
    pub sources: BTreeMap<String, SourceFile>,
    #[serde(default)]
    pub contracts: Contracts,
}

impl CompilerOutput {
    /// Whether the output contains a compiler error
    pub fn has_error(&self) -> bool {
        self.errors.iter().any(|err| err.severity.is_error())
    }

    /// Whether the output contains a compiler warning
    pub fn has_warning(&self, ignored_error_codes: &[u64]) -> bool {
        self.errors.iter().any(|err| {
            if err.severity.is_warning() {
                true
            } else {
                false
            }
        })
    }

    /// Finds the _first_ contract with the given name
    // pub fn find(&self, contract: impl AsRef<str>) -> Option<CompactContractRef> {
    //     let contract_name = contract.as_ref();
    //     self.contracts_iter().find_map(|(name, contract)| {
    //         (name == contract_name).then(|| CompactContractRef::from(contract))
    //     })
    // }

    /// Finds the first contract with the given name and removes it from the set
    pub fn remove(&mut self, contract: impl AsRef<str>) -> Option<Contract> {
        let contract_name = contract.as_ref();
        self.contracts.values_mut().find_map(|c| c.remove(contract_name))
    }

    /// Iterate over all contracts and their names
    pub fn contracts_iter(&self) -> impl Iterator<Item = (&String, &Contract)> {
        self.contracts.values().flatten()
    }

    /// Iterate over all contracts and their names
    pub fn contracts_into_iter(self) -> impl Iterator<Item = (String, Contract)> {
        self.contracts.into_values().flatten()
    }

    /// Given the contract file's path and the contract's name, tries to return the contract's
    /// bytecode, runtime bytecode, and abi
    // pub fn get(&self, path: &str, contract: &str) -> Option<CompactContractRef> {
    //     self.contracts
    //         .get(path)
    //         .and_then(|contracts| contracts.get(contract))
    //         .map(CompactContractRef::from)
    // }

    /// Returns the output's source files and contracts separately, wrapped in helper types that
    /// provide several helper methods
    // pub fn split(self) -> (SourceFiles, OutputContracts) {
    //     (SourceFiles(self.sources), OutputContracts(self.contracts))
    // }

    /// Retains only those files the given iterator yields
    ///
    /// In other words, removes all contracts for files not included in the iterator
    // pub fn retain_files<'a, I>(&mut self, files: I)
    //     where
    //         I: IntoIterator<Item = &'a str>,
    // {
    //     // Note: use `to_lowercase` here because solc not necessarily emits the exact file name,
    //     // e.g. `src/utils/upgradeProxy.sol` is emitted as `src/utils/UpgradeProxy.sol`
    //     let files: HashSet<_> = files.into_iter().map(|s| s.to_lowercase()).collect();
    //     self.contracts.retain(|f, _| files.contains(f.to_lowercase().as_str()));
    //     self.sources.retain(|f, _| files.contains(f.to_lowercase().as_str()));
    //     self.errors.retain(|err| {
    //         err.source_location
    //             .as_ref()
    //             .map(|s| files.contains(s.file.to_lowercase().as_str()))
    //             .unwrap_or(true)
    //     });
    // }

    pub fn merge(&mut self, other: CompilerOutput) {
        self.errors.extend(other.errors);
        self.contracts.extend(other.contracts);
        self.sources.extend(other.sources);
    }
}

pub async fn compile(vyper_path: &PathBuf, input: &CompilerInput, args: Option<&Vec<String>>) -> Result<CompilerOutput, VyperError> {
    let mut cmd = Command::new(vyper_path);
    if let Some(args) = args {
        cmd.args(args);
    }
    let mut child = cmd
        .arg("--standard-json")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| VyperError::vyper(err.to_string()))?;
    let stdin = child.stdin.take().expect("Stdin exists.");
    serde_json::to_writer(stdin, input)?;
    let output = child.wait_with_output().map_err(|err| VyperError::io(err, &vyper_path))?;
    if output.status.success() {
        Ok(serde_json::from_slice(&output.stdout)?)
    } else {
        Err(VyperError::vyper(String::from_utf8_lossy(&output.stderr).to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::{env, fs};
    use super::*;

    struct Input {
        source_code: String,
    }

    impl Input {
        pub fn with_source_code(source_code: String) -> Self {
            Self { source_code }
        }
    }

    impl From<Input> for CompilerInput {
        fn from(input: Input) -> Self {
            let mut compiler_input = CompilerInput {
                language: "Vyper".to_string(),
                sources: Sources::from([(
                    "src/source.vy".into(),
                    Source {
                        content: input.source_code,
                    },
                )]),
                settings: Default::default(),
            };
            compiler_input.settings.evm_version = None;
            compiler_input
        }
    }

    #[tokio::test]
    async fn successful_compilation() {
        let source_code = r#"
beneficiary: public(address)
auctionStart: public(uint256)
auctionEnd: public(uint256)

highestBidder: public(address)
highestBid: public(uint256)

ended: public(bool)

pendingReturns: public(HashMap[address, uint256])

@external
def __init__(_beneficiary: address, _auction_start: uint256, _bidding_time: uint256):
    self.beneficiary = _beneficiary
    self.auctionStart = _auction_start  # auction start time can be in the past, present or future
    self.auctionEnd = self.auctionStart + _bidding_time
    assert block.timestamp < self.auctionEnd # auction end time should be in the future

@external
@payable
def bid():
    assert block.timestamp >= self.auctionStart
    assert block.timestamp < self.auctionEnd
    assert msg.value > self.highestBid
    self.pendingReturns[self.highestBidder] += self.highestBid
    self.highestBidder = msg.sender
    self.highestBid = msg.value

@external
def withdraw():
    pending_amount: uint256 = self.pendingReturns[msg.sender]
    self.pendingReturns[msg.sender] = 0
    send(msg.sender, pending_amount)

@external
def endAuction():
    assert block.timestamp >= self.auctionEnd
    assert not self.ended
    self.ended = True
    send(self.beneficiary, self.highestBid)
        "#;

        let input: CompilerInput = Input::with_source_code(source_code.into()).into();
        let vyper_path = PathBuf::from("src/vyper/compiler_tests/vyper-0.3.6.exe");

        let result = compile(&vyper_path, &input, None)
            .await
            .expect("Compilation failed");
        assert!(
            !result.contracts.is_empty(),
            "Result should consists of at least one contract"
        );
    }
}
