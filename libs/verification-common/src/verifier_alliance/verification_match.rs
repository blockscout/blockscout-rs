use super::{
    compilation_artifacts::CompilationArtifacts, creation_code_artifacts::CreationCodeArtifacts,
    runtime_code_artifacts::RuntimeCodeArtifacts, CborAuxdata,
};
pub use super::{
    verification_match_transformations::Transformation as MatchTransformation,
    verification_match_values::Values as MatchValues,
};
use alloy_dyn_abi::JsonAbiExt;
use anyhow::Context;
use bytes::Bytes;
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub metadata_match: bool,
    pub transformations: Vec<MatchTransformation>,
    pub values: MatchValues,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchBuilder<'a> {
    deployed_code: &'a [u8],
    compiled_code: Vec<u8>,
    transformations: Vec<MatchTransformation>,
    values: MatchValues,
    invalid_constructor_arguments: bool,
    has_cbor_auxdata: bool,
    has_cbor_auxdata_transformation: bool,
}

impl<'a> MatchBuilder<'a> {
    pub fn new(deployed_code: &'a [u8], compiled_code: Vec<u8>) -> Option<Self> {
        if deployed_code.len() < compiled_code.len() {
            return None;
        }

        Some(Self {
            deployed_code,
            compiled_code,
            transformations: vec![],
            values: MatchValues::default(),
            invalid_constructor_arguments: false,
            has_cbor_auxdata: false,
            has_cbor_auxdata_transformation: false,
        })
    }

    pub fn set_has_cbor_auxdata(mut self, value: bool) -> Self {
        self.has_cbor_auxdata = value;
        self
    }

    pub fn apply_runtime_code_transformations(
        self,
        runtime_code_artifacts: &RuntimeCodeArtifacts,
    ) -> Result<Self, anyhow::Error> {
        self.apply_cbor_auxdata_transformations(runtime_code_artifacts.cbor_auxdata.as_ref())?
            .apply_library_transformations(runtime_code_artifacts.link_references.as_ref())?
            .apply_immutable_transformations(runtime_code_artifacts.immutable_references.as_ref())
    }

    pub fn apply_creation_code_transformations(
        self,
        creation_code_artifacts: &CreationCodeArtifacts,
        compilation_artifacts: &CompilationArtifacts,
    ) -> Result<Self, anyhow::Error> {
        self.apply_cbor_auxdata_transformations(creation_code_artifacts.cbor_auxdata.as_ref())?
            .apply_library_transformations(creation_code_artifacts.link_references.as_ref())?
            .apply_constructor_transformation(compilation_artifacts.abi.as_ref())
    }

    pub fn verify_and_build(self) -> Option<Match> {
        if !self.invalid_constructor_arguments
            && self.deployed_code == self.compiled_code.as_slice()
        {
            let metadata_match = self.has_cbor_auxdata && !self.has_cbor_auxdata_transformation;
            return Some(Match {
                metadata_match,
                transformations: self.transformations,
                values: self.values,
            });
        }

        None
    }

    fn apply_cbor_auxdata_transformations(
        mut self,
        cbor_auxdata: Option<&CborAuxdata>,
    ) -> Result<Self, anyhow::Error> {
        if let Some(cbor_auxdata) = cbor_auxdata {
            self.has_cbor_auxdata = !cbor_auxdata.is_empty();
        }
        Ok(self)
    }

    fn apply_library_transformations(
        self,
        _link_references: Option<&serde_json::Value>,
    ) -> Result<Self, anyhow::Error> {
        Ok(self)
    }

    fn apply_immutable_transformations(
        self,
        _immutable_references: Option<&serde_json::Value>,
    ) -> Result<Self, anyhow::Error> {
        Ok(self)
    }

    fn apply_constructor_transformation(
        mut self,
        abi: Option<&serde_json::Value>,
    ) -> Result<Self, anyhow::Error> {
        let offset = self.compiled_code.len();
        let (_prefix, constructor_arguments) = self.deployed_code.split_at(offset);

        let constructor = match abi {
            Some(abi) => {
                alloy_json_abi::JsonAbi::deserialize(abi)
                    .context("parsing compiled contract abi")?
                    .constructor
            }
            None => None,
        };

        match constructor {
            None if !constructor_arguments.is_empty() => {
                self.invalid_constructor_arguments = true;
            }
            Some(_constructor) if constructor_arguments.is_empty() => {
                self.invalid_constructor_arguments = true;
            }
            Some(constructor)
                if constructor
                    .abi_decode_input(constructor_arguments, true)
                    .is_err() =>
            {
                self.invalid_constructor_arguments = true;
            }
            None => {}
            Some(_constructor) => {
                self.compiled_code.extend(constructor_arguments);
                self.transformations
                    .push(MatchTransformation::constructor(offset));
                self.values
                    .add_constructor_arguments(Bytes::copy_from_slice(constructor_arguments));
            }
        }

        Ok(self)
    }
}
