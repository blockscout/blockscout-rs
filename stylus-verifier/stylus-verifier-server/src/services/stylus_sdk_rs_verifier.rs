use crate::settings::DockerApiSettings;
use async_trait::async_trait;
use stylus_verifier_logic::stylus_sdk_rs;
use stylus_verifier_proto::blockscout::stylus_verifier::v1::{
    stylus_sdk_rs_verifier_server::StylusSdkRsVerifier, verify_response, CargoStylusVersion,
    CargoStylusVersions, ListCargoStylusVersionsRequest, VerificationFailure,
    VerifyGithubRepositoryRequest, VerifyResponse,
};
use tonic::{Request, Response, Status};

pub struct StylusSdkRsVerifierService {
    docker_client: stylus_sdk_rs::Docker,
    supported_cargo_stylus_versions: Vec<semver::Version>,
}

impl StylusSdkRsVerifierService {
    pub async fn new(docker_api_settings: DockerApiSettings) -> Self {
        Self {
            docker_client: stylus_sdk_rs::docker_connect(&docker_api_settings.addr)
                .await
                .expect("failed to connect to docker daemon"),
            // TODO: to be automatically retrieved from the dockerhub registry
            supported_cargo_stylus_versions: vec![
                semver::Version::new(0, 5, 0),
                semver::Version::new(0, 5, 1),
                semver::Version::new(0, 5, 2),
                semver::Version::new(0, 5, 3),
            ],
        }
    }

    fn is_cargo_stylus_version_supported(&self, version: &semver::Version) -> bool {
        self.supported_cargo_stylus_versions.contains(version)
    }
}

#[async_trait]
impl StylusSdkRsVerifier for StylusSdkRsVerifierService {
    async fn verify_github_repository(
        &self,
        request: Request<VerifyGithubRepositoryRequest>,
    ) -> Result<Response<VerifyResponse>, Status> {
        let request = request.into_inner();
        let request: stylus_sdk_rs::VerifyGithubRepositoryRequest = match request.try_into() {
            Ok(request) => request,
            Err(err) => return process_error(err),
        };

        if !self.is_cargo_stylus_version_supported(&request.cargo_stylus_version) {
            return Err(Status::invalid_argument(format!(
                "cargo stylus version is not supported: {}",
                request.cargo_stylus_version
            )));
        }

        let result = stylus_sdk_rs::verify_github_repository(&self.docker_client, request).await;
        process_verify_result(result)
    }

    async fn list_cargo_stylus_versions(
        &self,
        _request: Request<ListCargoStylusVersionsRequest>,
    ) -> Result<Response<CargoStylusVersions>, Status> {
        let versions = self
            .supported_cargo_stylus_versions
            .iter()
            .map(|version| CargoStylusVersion {
                version: format!("v{version}"),
            })
            .collect();
        Ok(Response::new(CargoStylusVersions { versions }))
    }
}

fn process_verify_result(
    result: Result<stylus_sdk_rs::Success, stylus_sdk_rs::Error>,
) -> Result<Response<VerifyResponse>, Status> {
    match result {
        Ok(success) => {
            let verify_response =
                verify_response::VerifyResponse::VerificationSuccess(success.into());

            Ok(Response::new(VerifyResponse {
                verify_response: Some(verify_response),
            }))
        }
        Err(err) => process_error(err),
    }
}

fn process_error(error: stylus_sdk_rs::Error) -> Result<Response<VerifyResponse>, Status> {
    let verify_response = match error {
        stylus_sdk_rs::Error::VerificationFailed(_)
        | stylus_sdk_rs::Error::RepositoryIsNotGithub(_)
        | stylus_sdk_rs::Error::RepositoryNotFound(_)
        | stylus_sdk_rs::Error::CommitNotFound(_)
        | stylus_sdk_rs::Error::ToolchainNotFound
        | stylus_sdk_rs::Error::InvalidToolchain(_) => {
            verify_response::VerifyResponse::VerificationFailure(VerificationFailure {
                message: error.to_string(),
            })
        }
        stylus_sdk_rs::Error::BadRequest(_) => {
            return Err(Status::invalid_argument(error.to_string()))
        }
        stylus_sdk_rs::Error::Internal(_) => return Err(Status::internal(error.to_string())),
    };

    Ok(Response::new(VerifyResponse {
        verify_response: Some(verify_response),
    }))
}
