use crate::{metrics, verification_response::VerificationResponse, DisplayBytes};
use actix_web::{error, web, web::Json};
use ethers_solc::EvmVersion;
use serde::Deserialize;
use smart_contract_verifier::{vyper, VerificationError, Version, VyperClient};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use tracing::instrument;

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct VerificationRequest {
    pub deployed_bytecode: String,
    pub creation_bytecode: Option<String>,
    pub compiler_version: String,

    #[serde(flatten)]
    pub content: MultiPartFiles,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone)]
pub struct MultiPartFiles {
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: Option<String>,
}

impl TryFrom<VerificationRequest> for vyper::multi_part::VerificationRequest {
    type Error = actix_web::Error;

    fn try_from(value: VerificationRequest) -> Result<Self, Self::Error> {
        let deployed_bytecode = DisplayBytes::from_str(&value.deployed_bytecode)
            .map_err(|err| error::ErrorBadRequest(format!("Invalid deployed bytecode: {err}")))?
            .0;
        let creation_bytecode = match value.creation_bytecode {
            None => None,
            Some(creation_bytecode) => Some(
                DisplayBytes::from_str(&creation_bytecode)
                    .map_err(|err| {
                        error::ErrorBadRequest(format!("Invalid creation bytecode: {err:?}"))
                    })?
                    .0,
            ),
        };
        let compiler_version = Version::from_str(&value.compiler_version)
            .map_err(|err| error::ErrorBadRequest(format!("Invalid compiler version: {err}")))?;
        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: value.content.try_into()?,
            chain_id: Default::default(),
        })
    }
}

impl TryFrom<MultiPartFiles> for vyper::multi_part::MultiFileContent {
    type Error = actix_web::Error;

    fn try_from(value: MultiPartFiles) -> Result<Self, Self::Error> {
        let sources: BTreeMap<PathBuf, String> = value
            .sources
            .into_iter()
            .map(|(name, content)| (name, content))
            .collect();

        let evm_version = if let Some(version) = value.evm_version {
            Some(EvmVersion::from_str(&version).map_err(error::ErrorBadRequest)?)
        } else {
            None
        };

        Ok(Self {
            sources,
            interfaces: Default::default(),
            evm_version,
        })
    }
}

#[instrument(skip(client, params), level = "debug")]
pub async fn verify(
    client: web::Data<VyperClient>,
    params: Json<VerificationRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    let request = params.into_inner().try_into()?;

    let result = vyper::multi_part::verify(client.into_inner(), request).await;

    if let Ok(verification_success) = result {
        let response = VerificationResponse::ok(verification_success.into());
        metrics::count_verify_contract("vyper", &response.status, "multi-part");
        return Ok(Json(response));
    }

    let err = result.unwrap_err();
    match err {
        VerificationError::Compilation(_)
        | VerificationError::NoMatchingContracts
        | VerificationError::CompilerVersionMismatch(_) => Ok(Json(VerificationResponse::err(err))),
        VerificationError::Initialization(_) | VerificationError::VersionNotFound(_) => {
            Err(error::ErrorBadRequest(err))
        }
        VerificationError::Internal(_) => {
            tracing::error!("internal error: {err}");
            Err(error::ErrorInternalServerError(err))
        }
    }
}
