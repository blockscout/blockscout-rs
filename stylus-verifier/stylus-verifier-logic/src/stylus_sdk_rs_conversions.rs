use crate::stylus_sdk_rs::{Error, Success, VerifyGithubRepositoryRequest};
use semver::Version;
use std::path::PathBuf;
use stylus_verifier_proto::blockscout::stylus_verifier::v1;
use url::Url;

impl TryFrom<v1::VerifyGithubRepositoryRequest> for VerifyGithubRepositoryRequest {
    type Error = Error;

    fn try_from(value: v1::VerifyGithubRepositoryRequest) -> Result<Self, Self::Error> {
        let deployment_transaction =
            blockscout_display_bytes::decode_hex(&value.deployment_transaction).map_err(|err| {
                Error::BadRequest(format!("deployment_transaction is not valid hex: {err:#?}"))
            })?;

        let cargo_stylus_version =
            Version::parse(value.cargo_stylus_version.trim_start_matches('v')).map_err(|err| {
                Error::BadRequest(format!(
                    "cargo_stylus_version is not valid semver version: {err:#?}"
                ))
            })?;

        let repository_url = Url::parse(&value.repository_url).map_err(|err| {
            Error::BadRequest(format!("repository_url is not valid url: {err:#?}"))
        })?;

        Ok(Self {
            deployment_transaction: deployment_transaction.into(),
            rpc_endpoint: value.rpc_endpoint,
            cargo_stylus_version,
            repository_url,
            commit: value.commit,
            path_prefix: PathBuf::from(&value.path_prefix),
        })
    }
}

impl From<Success> for v1::VerificationSuccess {
    fn from(value: Success) -> Self {
        Self {
            abi: value.abi.map(|abi| abi.to_string()),
            contract_name: value.contract_name,
            files: value.files,
            package_name: value.package_name,
            cargo_stylus_version: format!("v{}", value.cargo_stylus_version),
            github_repository_metadata: Some(v1::verification_success::GithubRepositoryMetadata {
                repository_url: value.repository_url.to_string(),
                commit: value.commit,
                path_prefix: value.path_prefix.to_string_lossy().to_string(),
            }),
        }
    }
}
