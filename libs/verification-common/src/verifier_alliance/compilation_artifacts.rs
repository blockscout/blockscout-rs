use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ToCompilationArtifacts {
    fn abi(&self) -> Option<Value> {
        None
    }
    fn devdoc(&self) -> Option<Value> {
        None
    }
    fn userdoc(&self) -> Option<Value> {
        None
    }
    fn storage_layout(&self) -> Option<Value> {
        None
    }
}

impl<T: ToCompilationArtifacts> ToCompilationArtifacts for &T {
    fn abi(&self) -> Option<Value> {
        (*self).abi()
    }
    fn devdoc(&self) -> Option<Value> {
        (*self).devdoc()
    }
    fn userdoc(&self) -> Option<Value> {
        (*self).userdoc()
    }
    fn storage_layout(&self) -> Option<Value> {
        (*self).storage_layout()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompilationArtifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abi: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub devdoc: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub userdoc: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_layout: Option<Value>,
}

impl<T: ToCompilationArtifacts> From<T> for CompilationArtifacts {
    fn from(value: T) -> Self {
        Self {
            abi: value.abi(),
            devdoc: value.devdoc(),
            userdoc: value.userdoc(),
            storage_layout: value.storage_layout(),
        }
    }
}

impl From<CompilationArtifacts> for Value {
    fn from(value: CompilationArtifacts) -> Self {
        serde_json::to_value(value).expect("compilation artifacts serialization must succeed")
    }
}
