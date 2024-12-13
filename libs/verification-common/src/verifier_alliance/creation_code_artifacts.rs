use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ToCreationCodeArtifacts {
    fn cbor_auxdata(&self) -> Option<Value> {
        None
    }
    fn link_references(&self) -> Option<Value> {
        None
    }
    fn source_map(&self) -> Option<Value> {
        None
    }
}

impl<T: ToCreationCodeArtifacts> ToCreationCodeArtifacts for &T {
    fn cbor_auxdata(&self) -> Option<Value> {
        (*self).cbor_auxdata()
    }
    fn link_references(&self) -> Option<Value> {
        (*self).link_references()
    }
    fn source_map(&self) -> Option<Value> {
        (*self).source_map()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreationCodeArtifacts {
    pub source_map: Option<Value>,
    pub link_references: Option<Value>,
    pub cbor_auxdata: Option<Value>,
}

impl<T: ToCreationCodeArtifacts> From<T> for CreationCodeArtifacts {
    fn from(value: T) -> Self {
        Self {
            link_references: value.link_references(),
            source_map: value.source_map(),
            cbor_auxdata: value.cbor_auxdata(),
        }
    }
}

impl From<CreationCodeArtifacts> for Value {
    fn from(value: CreationCodeArtifacts) -> Self {
        serde_json::to_value(value).expect("creation code artifacts serialization must succeed")
    }
}
