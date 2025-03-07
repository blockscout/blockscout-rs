use verification_common::{
    verifier_alliance,
    verifier_alliance::{CompilationArtifacts, CreationCodeArtifacts, Match, RuntimeCodeArtifacts},
};

#[derive(Clone, Debug)]
pub struct OnChainCode {
    runtime: Option<Vec<u8>>,
    creation: Option<Vec<u8>>,
}

impl OnChainCode {
    pub fn runtime(runtime_code: Vec<u8>) -> Self {
        Self {
            runtime: Some(runtime_code),
            creation: None,
        }
    }

    pub fn creation(creation_code: Vec<u8>) -> Self {
        Self {
            runtime: None,
            creation: Some(creation_code),
        }
    }

    pub fn complete(runtime_code: Vec<u8>, creation_code: Vec<u8>) -> Self {
        Self {
            runtime: Some(runtime_code),
            creation: Some(creation_code),
        }
    }
}

#[derive(Clone, Debug, PartialOrd, PartialEq)]
pub struct RecompiledCode {
    pub runtime: Vec<u8>,
    pub creation: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum VerificationResult {
    Failure,
    RuntimeMatch {
        runtime_match: Match,
    },
    CreationMatch {
        creation_match: Match,
    },
    CompleteMatch {
        runtime_match: Match,
        creation_match: Match,
    },
}

pub fn verify_contract(
    on_chain_code: OnChainCode,
    recompiled_code: RecompiledCode,
    compilation_artifacts: CompilationArtifacts,
    creation_code_artifacts: CreationCodeArtifacts,
    runtime_code_artifacts: RuntimeCodeArtifacts,
) -> VerificationResult {
    if on_chain_code.runtime.is_none() && on_chain_code.creation.is_none() {
        unreachable!("OnChainCode constructors require at least one of the code values")
    }

    let mut runtime_match = None;
    if let Some(on_chain_runtime_code) = &on_chain_code.runtime {
        let verify_code_result = verifier_alliance::verify_runtime_code(
            on_chain_runtime_code,
            recompiled_code.runtime,
            &runtime_code_artifacts,
        );
        runtime_match = process_verify_code_result("runtime", verify_code_result);
    }

    let mut creation_match = None;
    if let Some(on_chain_creation_code) = &on_chain_code.creation {
        let verify_code_result = verifier_alliance::verify_creation_code(
            on_chain_creation_code,
            recompiled_code.creation,
            &creation_code_artifacts,
            &compilation_artifacts,
        );
        creation_match = process_verify_code_result("creation", verify_code_result);
    }

    matches_to_verification_result(runtime_match, creation_match)
}

fn process_verify_code_result(
    code_type: &'static str,
    verification_result: Result<Option<Match>, anyhow::Error>,
) -> Option<Match> {
    match verification_result {
        Err(err) => {
            tracing::error!("({code_type} code) error while verifying: {err:#?}");
            None
        }
        Ok(None) => {
            tracing::debug!("({code_type} code) verification failed");
            None
        }
        Ok(Some(match_)) => Some(match_),
    }
}

fn matches_to_verification_result(
    runtime_match: Option<Match>,
    creation_match: Option<Match>,
) -> VerificationResult {
    match (runtime_match, creation_match) {
        (None, None) => VerificationResult::Failure,
        (Some(runtime_match), None) => VerificationResult::RuntimeMatch { runtime_match },
        (None, Some(creation_match)) => VerificationResult::CreationMatch { creation_match },
        (Some(runtime_match), Some(creation_match)) => VerificationResult::CompleteMatch {
            runtime_match,
            creation_match,
        },
    }
}
