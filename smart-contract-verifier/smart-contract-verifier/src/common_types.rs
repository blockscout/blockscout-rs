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
