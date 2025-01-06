use super::artifacts::CompilerInput;
use crate::{
    compiler,
    verifier::{self, LocalBytecodeParts},
    MatchType,
};
use blockscout_display_bytes::Bytes as DisplayBytes;
use ethers_solc::CompilerOutput;

#[derive(Clone, Debug)]
pub struct Success {
    pub compiler_input: CompilerInput,
    pub compiler_output: CompilerOutput,
    pub compiler_version: compiler::DetailedVersion,
    pub file_path: String,
    pub contract_name: String,
    pub abi: Option<serde_json::Value>,
    pub constructor_args: Option<DisplayBytes>,
    pub local_bytecode_parts: LocalBytecodeParts,
    pub match_type: MatchType,
    pub compilation_artifacts: serde_json::Value,
    pub creation_input_artifacts: serde_json::Value,
    pub deployed_bytecode_artifacts: serde_json::Value,
    pub is_blueprint: bool,
}

impl From<(CompilerInput, verifier::Success)> for Success {
    fn from((compiler_input, success): (CompilerInput, verifier::Success)) -> Self {
        Self {
            compiler_input,
            compiler_output: success.compiler_output,
            compiler_version: success.compiler_version,
            file_path: success.file_path,
            contract_name: success.contract_name,
            abi: success.abi,
            constructor_args: success.constructor_args,
            local_bytecode_parts: success.local_bytecode_parts,
            match_type: success.match_type,
            compilation_artifacts: success.compilation_artifacts,
            creation_input_artifacts: success.creation_input_artifacts,
            deployed_bytecode_artifacts: success.deployed_bytecode_artifacts,
            is_blueprint: success.is_blueprint,
        }
    }
}
