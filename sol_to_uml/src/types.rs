use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Deserialize, PartialEq)]
pub struct SolToUmlRequest {
    pub sources: BTreeMap<PathBuf, String>,
}

#[derive(Debug, Serialize)]
pub struct SolToUmlResponse {
    pub uml_diagram: String,
}
