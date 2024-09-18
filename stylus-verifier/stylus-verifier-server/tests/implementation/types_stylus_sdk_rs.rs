use blockscout_display_bytes::ToHex;
use bytes::Bytes;
use pretty_assertions::assert_eq;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::PathBuf};
use stylus_verifier_proto::blockscout::stylus_verifier::v1::{
    verify_response, VerificationSuccess, VerifyGithubRepositoryRequest, VerifyResponse,
};
use url::Url;

#[serde_with::serde_as]
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct VerifyGithubRepositoryTestCase {
    #[serde_as(as = "blockscout_display_bytes::serde_as::Hex")]
    pub deployment_transaction: Bytes,
    pub rpc_endpoint: Url,
    pub cargo_stylus_version: String,
    pub repository_url: Url,
    pub commit: String,
    pub path_prefix: String,

    #[serde_as(as = "serde_with::json::JsonString")]
    pub expected_abi: serde_json::Value,
    pub expected_contract_name: String,
    pub expected_files: BTreeMap<String, String>,
}

impl VerifyGithubRepositoryTestCase {
    pub fn from_file(test_case: &str) -> Self {
        let current_dir = std::env::current_dir().unwrap();
        let current_dir = current_dir.to_string_lossy();
        let test_case_path = PathBuf::from(format!(
            "{current_dir}/tests/test_cases_stylus_sdk_rs/{test_case}.json"
        ));
        let content = fs::read_to_string(test_case_path).expect("failed to read file");
        serde_json::from_str(&content).expect("invalid test case format")
    }

    pub fn to_request(&self) -> VerifyGithubRepositoryRequest {
        VerifyGithubRepositoryRequest {
            deployment_transaction: ToHex::to_hex(&self.deployment_transaction),
            rpc_endpoint: self.rpc_endpoint.to_string(),
            cargo_stylus_version: self.cargo_stylus_version.clone(),
            repository_url: self.repository_url.to_string(),
            commit: self.commit.clone(),
            path_prefix: self.path_prefix.clone(),
        }
    }

    pub fn check_verification_success(&self, success: VerificationSuccess) {
        assert_eq!(
            Some(&self.expected_contract_name),
            success.contract_name.as_ref(),
            "invalid contract name"
        );
        let abi = success.abi.map(|abi| {
            serde_json::from_str::<serde_json::Value>(&abi)
                .expect("invalid abi: cannot parse as json value")
        });
        assert_eq!(Some(&self.expected_abi), abi.as_ref(), "invalid abi");
        assert_eq!(self.expected_files, success.files, "invalid files");
        assert_eq!(
            self.cargo_stylus_version, success.cargo_stylus_version,
            "invalid cargo stylus version"
        );

        let github_repository_metadata = success
            .github_repository_metadata
            .expect("github_repository_metadata is absent");
        assert_eq!(
            self.repository_url.to_string(),
            github_repository_metadata.repository_url,
            "invalid repository url"
        );
        assert_eq!(
            self.commit, github_repository_metadata.commit,
            "invalid commit"
        );
        assert_eq!(
            self.path_prefix, github_repository_metadata.path_prefix,
            "invalid path prefix"
        );
    }

    pub fn check_verify_response(&self, response: VerifyResponse) {
        match response.verify_response {
            Some(response) => match response {
                verify_response::VerifyResponse::VerificationSuccess(success) => {
                    self.check_verification_success(success)
                }
                verify_response::VerifyResponse::VerificationFailure(failure) => {
                    panic!("invalid response (success expected): {failure:?}")
                }
            },
            None => panic!("invalid response: {response:?}"),
        }
    }
}
