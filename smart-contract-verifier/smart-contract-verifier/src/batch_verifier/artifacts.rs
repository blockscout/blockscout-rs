use std::collections::BTreeMap;

pub type LinkReferences =
    BTreeMap<String, BTreeMap<String, Vec<foundry_compilers::artifacts::Offsets>>>;

pub mod cbor_auxdata {
    use crate::BytecodePart;
    use blockscout_display_bytes::Bytes as DisplayBytes;
    use std::collections::BTreeMap;

    pub type CborAuxdata = BTreeMap<String, CborAuxdataValue>;

    #[derive(Clone, Debug, serde::Serialize, Eq, PartialEq)]
    pub struct CborAuxdataValue {
        offset: usize,
        value: DisplayBytes,
    }

    pub fn generate(bytecode_parts: &[BytecodePart]) -> CborAuxdata {
        let mut auxdata = BTreeMap::new();
        let mut offset = 0;
        for part in bytecode_parts {
            match part {
                BytecodePart::Main { .. } => offset += part.size(),
                BytecodePart::Metadata { raw, .. } => {
                    let id = format!("{}", auxdata.len() + 1);
                    let value = DisplayBytes::from(raw.to_vec());
                    auxdata.insert(id, CborAuxdataValue { offset, value });
                    offset += part.size();
                }
            }
        }
        auxdata
    }
}

pub mod compilation_artifacts {
    use crate::verifier::lossless_compiler_output;

    #[derive(Clone, Debug, serde::Serialize, Eq, PartialEq)]
    #[serde(rename_all = "camelCase")]
    // We need a separate structure, as `artifacts::SourceFile` does include
    // serialization of "ast" field even though it contains `None` value.
    pub struct SourceFile {
        id: u32,
    }

    #[derive(Clone, Debug, serde::Serialize, Eq, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct CompilationArtifacts {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub abi: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub devdoc: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub userdoc: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub storage_layout: Option<serde_json::Value>,
        pub sources: serde_json::Value,
    }

    pub fn generate(
        contract: &lossless_compiler_output::Contract,
        source_files: &lossless_compiler_output::SourceFiles,
    ) -> CompilationArtifacts {
        CompilationArtifacts {
            abi: contract.abi.clone(),
            devdoc: contract.devdoc.clone(),
            userdoc: contract.userdoc.clone(),
            storage_layout: contract.storage_layout.clone(),
            sources: source_files
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        serde_json::to_value(SourceFile { id: v.id }).unwrap(),
                    )
                })
                .collect(),
        }
    }
}

pub mod creation_code_artifacts {
    use super::*;
    use crate::verifier::lossless_compiler_output;
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, serde::Serialize, Eq, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct CreationCodeArtifacts {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source_map: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub link_references: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        pub cbor_auxdata: cbor_auxdata::CborAuxdata,
    }

    pub fn generate(
        raw_contract: &lossless_compiler_output::Contract,
        cbor_auxdata: cbor_auxdata::CborAuxdata,
    ) -> CreationCodeArtifacts {
        let bytecode = &raw_contract.evm.bytecode;
        CreationCodeArtifacts {
            source_map: bytecode.source_map.clone(),
            link_references: bytecode.link_references.clone(),
            cbor_auxdata,
        }
    }
}

pub mod runtime_code_artifacts {
    use crate::verifier::lossless_compiler_output;
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, serde::Serialize, Eq, PartialEq)]
    #[serde(rename_all = "camelCase")]
    pub struct RuntimeCodeArtifacts {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source_map: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub link_references: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub immutable_references: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "BTreeMap::is_empty")]
        pub cbor_auxdata: super::cbor_auxdata::CborAuxdata,
    }

    pub fn generate(
        raw_contract: &lossless_compiler_output::Contract,
        cbor_auxdata: super::cbor_auxdata::CborAuxdata,
    ) -> RuntimeCodeArtifacts {
        let deployed_bytecode = &raw_contract.evm.deployed_bytecode;
        RuntimeCodeArtifacts {
            source_map: deployed_bytecode.bytecode.source_map.clone(),
            link_references: deployed_bytecode.bytecode.link_references.clone(),
            immutable_references: deployed_bytecode.immutable_references.clone(),
            cbor_auxdata,
        }
    }
}
