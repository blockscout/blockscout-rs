mod action_helpers;

pub use action_helpers::*;

/**********************************************/

#[derive(Clone, Debug)]
pub struct CodeMatch {
    pub does_match: bool,
    pub values: Option<serde_json::Value>,
    pub transformations: Option<serde_json::Value>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum TransformationStatus {
    NoMatch,
    WithAuxdata,
    WithoutAuxdata,
}
