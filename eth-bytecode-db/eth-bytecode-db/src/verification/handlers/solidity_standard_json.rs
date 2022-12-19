use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::VerifySolidityStandardJsonRequest,
        types::{BytecodeType, Source, SourceType, VerificationRequest, VerificationType},
    },
    process_verify_response, ProcessResponseAction,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardJson {
    pub input: String,
}

impl From<VerificationRequest<StandardJson>> for VerifySolidityStandardJsonRequest {
    fn from(request: VerificationRequest<StandardJson>) -> Self {
        let (creation_bytecode, deployed_bytecode) = match request.bytecode_type {
            BytecodeType::CreationInput => (Some(request.bytecode), "".to_string()),
            BytecodeType::DeployedBytecode => (None, request.bytecode),
        };
        Self {
            creation_bytecode,
            deployed_bytecode,
            compiler_version: request.compiler_version,
            input: request.content.input,
        }
    }
}

pub async fn verify(
    mut client: Client,
    request: VerificationRequest<StandardJson>,
) -> Result<Source, Error> {
    let bytecode_type = request.bytecode_type;
    let raw_request_bytecode = hex::decode(request.bytecode.clone().trim_start_matches("0x"))
        .map_err(|err| Error::InvalidArgument(format!("invalid bytecode: {}", err)))?;
    let verification_settings = serde_json::json!(&request);

    let request: VerifySolidityStandardJsonRequest = request.into();
    let response = client
        .solidity_client
        .verify_standard_json(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    let source_type_fn = |file_name: &str| {
        if file_name.ends_with(".sol") {
            Ok(SourceType::Solidity)
        } else if file_name.ends_with(".yul") {
            Ok(SourceType::Yul)
        } else {
            Err(Error::Internal(
                anyhow::anyhow!(
                    "unknown verified file extension: expected \".sol\" or \".yul\"; file_name={}",
                    file_name
                )
                .context("verifier service connection"),
            ))
        }
    };

    process_verify_response(
        &client.db_client,
        response,
        source_type_fn,
        ProcessResponseAction::SaveData {
            bytecode_type,
            raw_request_bytecode,
            verification_settings,
            verification_type: VerificationType::StandardJson,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn from_verification_request_creation_input() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput,
            compiler_version: "compiler_version".to_string(),
            content: StandardJson {
                input: "standard_json_input".to_string(),
            },
        };
        let expected = VerifySolidityStandardJsonRequest {
            creation_bytecode: Some("0x1234".to_string()),
            deployed_bytecode: "".to_string(),
            compiler_version: "compiler_version".to_string(),
            input: "standard_json_input".to_string(),
        };
        assert_eq!(
            expected,
            VerifySolidityStandardJsonRequest::from(request),
            "Invalid conversion"
        );
    }

    #[test]
    fn from_verification_request_deployed_bytecode() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::DeployedBytecode,
            compiler_version: "compiler_version".to_string(),
            content: StandardJson {
                input: "standard_json_input".to_string(),
            },
        };
        let expected = VerifySolidityStandardJsonRequest {
            creation_bytecode: None,
            deployed_bytecode: "0x1234".to_string(),
            compiler_version: "compiler_version".to_string(),
            input: "standard_json_input".to_string(),
        };
        assert_eq!(
            expected,
            VerifySolidityStandardJsonRequest::from(request),
            "Invalid conversion"
        );
    }
}
