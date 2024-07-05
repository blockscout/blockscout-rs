use serde::{Deserialize, Serialize};
use serde_json::Value;

pub trait ToRuntimeCodeArtifacts {
    fn cbor_auxdata(&self) -> Option<Value> {
        None
    }
    fn immutable_references(&self) -> Option<Value> {
        None
    }
    fn link_references(&self) -> Option<Value> {
        None
    }
    fn source_map(&self) -> Option<Value> {
        None
    }
}

impl<T: ToRuntimeCodeArtifacts> ToRuntimeCodeArtifacts for &T {
    fn cbor_auxdata(&self) -> Option<Value> {
        (*self).cbor_auxdata()
    }
    fn immutable_references(&self) -> Option<Value> {
        (*self).immutable_references()
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
pub struct RuntimeCodeArtifacts {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cbor_auxdata: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub immutable_references: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_references: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_map: Option<Value>,
}

impl<T: ToRuntimeCodeArtifacts> From<T> for RuntimeCodeArtifacts {
    fn from(value: T) -> Self {
        Self {
            cbor_auxdata: value.cbor_auxdata(),
            immutable_references: value.immutable_references(),
            link_references: value.link_references(),
            source_map: value.source_map(),
        }
    }
}

impl From<(RuntimeCodeArtifacts, RuntimeCodeArtifacts)> for RuntimeCodeArtifacts {
    fn from(
        (base_artifacts, merged_artifacts): (RuntimeCodeArtifacts, RuntimeCodeArtifacts),
    ) -> Self {
        Self {
            cbor_auxdata: merged_artifacts
                .cbor_auxdata
                .or(base_artifacts.cbor_auxdata),
            immutable_references: merged_artifacts
                .immutable_references
                .or(base_artifacts.immutable_references),
            link_references: merged_artifacts
                .link_references
                .or(base_artifacts.link_references),
            source_map: merged_artifacts.source_map.or(base_artifacts.source_map),
        }
    }
}

impl From<RuntimeCodeArtifacts> for Value {
    fn from(value: RuntimeCodeArtifacts) -> Self {
        serde_json::to_value(value).expect("runtime code artifacts serialization must succeed")
    }
}
