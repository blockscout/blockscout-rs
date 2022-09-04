use crate::{
    handlers::verification_response::{VerificationResponse, VerificationResult},
    DisplayBytes,
};
use actix_web::{error, web, web::Json};
use ethers_solc::EvmVersion;
use serde::Deserialize;
use smart_contract_verifier::{solidity, Compilers, SolidityCompiler, Version};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use tracing::instrument;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: String,
    pub creation_bytecode: String,
    pub compiler_version: String,

    #[serde(flatten)]
    pub content: MultiPartFiles,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct MultiPartFiles {
    pub sources: BTreeMap<PathBuf, String>,
    pub evm_version: String,
    pub optimization_runs: Option<usize>,
    pub contract_libraries: Option<BTreeMap<String, String>>,
}

impl TryFrom<VerificationRequest> for solidity::multi_part::VerificationRequest {
    type Error = actix_web::Error;

    fn try_from(value: VerificationRequest) -> Result<Self, Self::Error> {
        let deployed_bytecode = DisplayBytes::from_str(&value.deployed_bytecode)
            .map_err(|err| error::ErrorBadRequest(format!("Invalid deployed bytecode: {:?}", err)))?
            .0;
        let creation_bytecode = DisplayBytes::from_str(&value.creation_bytecode)
            .map_err(|err| error::ErrorBadRequest(format!("Invalid creation bytecode: {:?}", err)))?
            .0;
        let compiler_version = Version::from_str(&value.compiler_version)
            .map_err(|err| error::ErrorBadRequest(format!("Invalid compiler version: {}", err)))?;
        Ok(Self {
            deployed_bytecode,
            creation_bytecode,
            compiler_version,
            content: value.content.try_into()?,
        })
    }
}

impl TryFrom<MultiPartFiles> for solidity::multi_part::MultiFileContent {
    type Error = actix_web::Error;

    fn try_from(value: MultiPartFiles) -> Result<Self, Self::Error> {
        let sources: BTreeMap<PathBuf, String> = value
            .sources
            .into_iter()
            .map(|(name, content)| (name, content))
            .collect();

        let evm_version = if value.evm_version != "default" {
            Some(EvmVersion::from_str(&value.evm_version).map_err(error::ErrorBadRequest)?)
        } else {
            None
        };

        Ok(Self {
            sources,
            evm_version,
            optimization_runs: value.optimization_runs,
            contract_libraries: value.contract_libraries,
        })
    }
}

#[instrument(skip(compilers, params), level = "debug")]
pub async fn verify(
    compilers: web::Data<Compilers<SolidityCompiler>>,
    params: Json<VerificationRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    let request = params.into_inner().try_into()?;

    let result = solidity::multi_part::verify(compilers.into_inner(), request).await;

    if let Ok(verification_success) = result {
        return Ok(Json(VerificationResponse::ok(verification_success.into())));
    }

    let err = result.unwrap_err();
    return match err {
        solidity::Error::Compilation(_) | solidity::Error::NoMatchingContracts => {
            Ok(Json(VerificationResponse::err(err)))
        }
        solidity::Error::Initialization(_) | solidity::Error::VersionNotFound(_) => {
            Err(error::ErrorBadRequest(err))
        }
        solidity::Error::Internal(_) => Err(error::ErrorInternalServerError(err)),
    };
}
