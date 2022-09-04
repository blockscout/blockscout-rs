use crate::{verification_response::VerificationResponse, DisplayBytes};
use actix_web::{error, web, web::Json};
use anyhow::anyhow;
use ethers_solc::CompilerInput;
use serde::Deserialize;
use smart_contract_verifier::{solidity, Compilers, SolidityCompiler, VerificationError, Version};
use std::str::FromStr;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Deserialize)]
pub struct VerificationRequest {
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,

    #[serde(flatten)]
    pub content: StandardJson,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StandardJson {
    input: String,
}

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("content is not valid standard json: {0}")]
    InvalidContent(#[from] serde_json::Error),
    #[error("{0}")]
    BadRequest(#[from] anyhow::Error),
}

impl TryFrom<VerificationRequest> for solidity::standard_json::VerificationRequest {
    type Error = ParseError;

    fn try_from(value: VerificationRequest) -> Result<Self, Self::Error> {
        let deployed_bytecode = DisplayBytes::from_str(&value.deployed_bytecode)
            .map_err(|err| anyhow!("Invalid deployed bytecode: {:?}", err))?
            .0;
        let creation_bytecode = DisplayBytes::from_str(&value.creation_bytecode)
            .map_err(|err| anyhow!("Invalid creation bytecode: {:?}", err))?
            .0;
        let compiler_version = Version::from_str(&value.compiler_version)
            .map_err(|err| anyhow!("Invalid compiler version: {}", err))?;
        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: value.content.try_into()?,
        })
    }
}

impl TryFrom<StandardJson> for solidity::standard_json::StandardJsonContent {
    type Error = ParseError;

    fn try_from(value: StandardJson) -> Result<Self, Self::Error> {
        let input: CompilerInput = serde_json::from_str(&value.input)?;

        Ok(Self { input })
    }
}

#[instrument(skip(compilers, params), level = "debug")]
pub async fn verify(
    compilers: web::Data<Compilers<SolidityCompiler>>,
    params: Json<VerificationRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    let request = {
        let request: Result<_, ParseError> = params.into_inner().try_into();
        if let Err(err) = request {
            match err {
                ParseError::InvalidContent(_) => return Err(error::ErrorBadRequest(err)),
                ParseError::BadRequest(_) => return Ok(Json(VerificationResponse::err(err))),
            }
        }
        request.unwrap()
    };

    let result = solidity::standard_json::verify(compilers.into_inner(), request).await;

    if let Ok(verification_success) = result {
        return Ok(Json(VerificationResponse::ok(verification_success.into())));
    }

    let err = result.unwrap_err();
    match err {
        VerificationError::Compilation(_) | VerificationError::NoMatchingContracts => {
            Ok(Json(VerificationResponse::err(err)))
        }
        VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
            Err(error::ErrorBadRequest(err))
        }
        VerificationError::Internal(_) => Err(error::ErrorInternalServerError(err)),
    }
}
