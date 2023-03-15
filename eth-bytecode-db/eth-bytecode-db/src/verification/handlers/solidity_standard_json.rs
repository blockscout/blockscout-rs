use super::{
    super::{
        client::Client,
        errors::Error,
        smart_contract_verifier::{BytecodeType, VerifySolidityStandardJsonRequest},
        types::{Source, VerificationRequest, VerificationType},
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
        Self {
            bytecode: request.bytecode,
            bytecode_type: BytecodeType::from(request.bytecode_type).into(),
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
        .map_err(|err| Error::InvalidArgument(format!("invalid bytecode: {err}")))?;
    let verification_settings = serde_json::json!(&request);
    let verification_metadata = request.metadata.clone();

    let request: VerifySolidityStandardJsonRequest = request.into();
    let response = client
        .solidity_client
        .verify_standard_json(request)
        .await
        .map_err(Error::from)?
        .into_inner();

    process_verify_response(
        &client.db_client,
        response,
        ProcessResponseAction::SaveData {
            bytecode_type,
            raw_request_bytecode,
            verification_settings,
            verification_type: VerificationType::StandardJson,
            verification_metadata,
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::{super::super::types, *};
    use pretty_assertions::assert_eq;

    #[test]
    fn from_verification_request_creation_input() {
        let request = VerificationRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: types::BytecodeType::CreationInput,
            compiler_version: "compiler_version".to_string(),
            content: StandardJson {
                input: "standard_json_input".to_string(),
            },
            metadata: None,
        };
        let expected = VerifySolidityStandardJsonRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::CreationInput.into(),
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
            bytecode_type: types::BytecodeType::DeployedBytecode,
            compiler_version: "compiler_version".to_string(),
            content: StandardJson {
                input: "standard_json_input".to_string(),
            },
            metadata: None,
        };
        let expected = VerifySolidityStandardJsonRequest {
            bytecode: "0x1234".to_string(),
            bytecode_type: BytecodeType::DeployedBytecode.into(),
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
