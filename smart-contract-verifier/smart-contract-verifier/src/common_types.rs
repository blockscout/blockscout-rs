use std::fmt::{Display, Formatter};

/// The enum representing how provided bytecode corresponds
/// to the local result of source codes compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchType {
    Partial,
    Full,
}

impl From<sourcify::MatchType> for MatchType {
    fn from(value: sourcify::MatchType) -> Self {
        match value {
            sourcify::MatchType::Full => MatchType::Full,
            sourcify::MatchType::Partial => MatchType::Partial,
        }
    }
}

pub struct Contract {
    pub creation_code: Option<Vec<u8>>,
    pub runtime_code: Option<Vec<u8>>,
}

#[derive(Clone, Debug, PartialOrd, PartialEq, Hash, Eq, Ord)]
pub struct FullyQualifiedName {
    file_name: String,
    contract_name: String,
}

impl Display for FullyQualifiedName {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.fully_qualified_name())
    }
}

impl FullyQualifiedName {
    pub fn from_file_and_contract_names(file_name: String, contract_name: String) -> Self {
        Self {
            file_name,
            contract_name,
        }
    }

    pub fn fully_qualified_name(&self) -> String {
        format!("{}:{}", &self.file_name, &self.contract_name)
    }

    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    pub fn contract_name(&self) -> String {
        self.contract_name.clone()
    }
}

#[derive(Clone, Copy, Debug, PartialOrd, PartialEq, Hash, Eq, Ord)]
pub enum Language {
    Solidity,
    Yul,
    Vyper,
}
