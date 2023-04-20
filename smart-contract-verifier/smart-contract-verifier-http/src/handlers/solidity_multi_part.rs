use crate::{metrics, verification_response::VerificationResponse, DisplayBytes};
use actix_web::{error, web, web::Json};
use ethers_solc::EvmVersion;
use serde::Deserialize;
use smart_contract_verifier::{solidity, SolidityClient, VerificationError, Version};
use std::{collections::BTreeMap, path::PathBuf, str::FromStr};
use tracing::instrument;

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub struct VerificationRequest {
    pub deployed_bytecode: String,
    pub creation_bytecode: Option<String>,
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
            .map_err(|err| error::ErrorBadRequest(format!("Invalid deployed bytecode: {err:?}")))?
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

#[instrument(skip(client, params), level = "debug")]
pub async fn verify(
    client: web::Data<SolidityClient>,
    params: Json<VerificationRequest>,
) -> Result<Json<VerificationResponse>, actix_web::Error> {
    let request = params.into_inner().try_into()?;

    let result = solidity::multi_part::verify(client.into_inner(), request).await;

    if let Ok(verification_success) = result {
        let response = VerificationResponse::ok(verification_success.into());
        metrics::count_verify_contract("solidity", &response.status, "multi-part");
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
        VerificationError::Internal(_) => Err(error::ErrorInternalServerError(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::parse::test_deserialize_ok;
    use pretty_assertions::assert_eq;

    fn sources(sources: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
        sources
            .iter()
            .map(|(name, content)| (PathBuf::from(name), content.to_string()))
            .collect()
    }

    #[test]
    fn parse_multi_part() {
        test_deserialize_ok(vec![
            (
                r#"{
                        "deployed_bytecode": "0x6001",
                        "creation_bytecode": "0x6001",
                        "compiler_version": "0.8.3",
                        "sources": {
                            "source.sol": "pragma"
                        },
                        "evm_version": "london",
                        "optimization_runs": 200
                    }"#,
                VerificationRequest {
                    deployed_bytecode: "0x6001".into(),
                    creation_bytecode: Some("0x6001".into()),
                    compiler_version: "0.8.3".into(),
                    content: MultiPartFiles {
                        sources: sources(&[("source.sol", "pragma")]),
                        evm_version: format!("{}", EvmVersion::London),
                        optimization_runs: Some(200),
                        contract_libraries: None,
                    },
                },
            ),
            (
                r#"{
                    "deployed_bytecode": "0x6001",
                    "creation_bytecode": "0x6001",
                    "compiler_version": "0.8.3",
                    "sources": {
                        "source.sol": "source",
                        "A.sol": "A",
                        "B": "B",
                        "metadata.json": "metadata"
                    },
                    "evm_version": "spuriousDragon",
                    "contract_libraries": {
                        "Lib.sol": "0x1234567890123456789012345678901234567890"
                    }
                }"#,
                VerificationRequest {
                    deployed_bytecode: "0x6001".into(),
                    creation_bytecode: Some("0x6001".into()),
                    compiler_version: "0.8.3".into(),
                    content: MultiPartFiles {
                        sources: sources(&[
                            ("source.sol", "source"),
                            ("A.sol", "A"),
                            ("B", "B"),
                            ("metadata.json", "metadata"),
                        ]),
                        evm_version: format!("{}", ethers_solc::EvmVersion::SpuriousDragon),
                        optimization_runs: None,
                        contract_libraries: Some(BTreeMap::from([(
                            "Lib.sol".into(),
                            "0x1234567890123456789012345678901234567890".into(),
                        )])),
                    },
                },
            ),
        ])
    }

    #[test]
    // 'default' should result in None in MultiFileContent
    fn default_evm_version() {
        let multi_part = MultiPartFiles {
            sources: BTreeMap::new(),
            evm_version: "default".to_string(),
            optimization_runs: None,
            contract_libraries: None,
        };
        let content = solidity::multi_part::MultiFileContent::try_from(multi_part)
            .expect("Structure is valid");
        assert_eq!(
            None, content.evm_version,
            "'default' should result in `None`"
        )
    }
}
