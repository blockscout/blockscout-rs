// SPDX-License-Identifier: LicenseRef-Blockscout

use crate::types_stylus_sdk_rs::VerifyGithubRepositoryTestCase;
use stylus_verifier_proto::blockscout::stylus_verifier::v1::VerifyResponse;

use url::Url;

const VERIFY_GITHUB_REPOSITORY_ROUTE: &str = "/api/v1/stylus-sdk-rs:verify-github-repository";

const LIST_CARGO_STYLUS_VERSIONS_ROUTE: &str = "/api/v1/stylus-sdk-rs/cargo-stylus-versions";

mod verify_github_repository {
    use super::*;

    async fn test(test_name: &str) {
        let test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(test_name);

        let server = crate::start_server().await;

        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;

        test_case.check_verify_response(response);
    }

    // Verification compares the output of `cargo stylus get-initcode`, which exists from 0.5.12.
    #[tokio::test]
    #[rstest::rstest]
    #[case::zero_five_twelve("0.5.12")]
    #[case::zero_six_zero("0.6.0")]
    #[case::zero_six_one("0.6.1")]
    #[case::zero_six_two("0.6.2")]
    #[case::zero_six_three("0.6.3")]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn with_prefix(#[case] stylus_version: &str) {
        test(&format!(
            "verify_github_repository_single_call_with_prefix_{stylus_version}"
        ))
        .await
    }

    // A contract on Arbitrum One deployed with 0.10.7 and Rust 1.97.1. From 0.10 the build runs
    // at the project's workspace root, which here is the repository root with a member crate.
    #[tokio::test]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn zero_ten_from_the_workspace_root() {
        test("verify_github_repository_vess_contract_0.10.7").await
    }
}

mod list_cargo_stylus_versions {
    use super::*;
    use stylus_verifier_proto::blockscout::stylus_verifier::v1::CargoStylusVersions;

    #[tokio::test]
    async fn contains_at_least_0_5_0() {
        let server = crate::start_server().await;

        let response: CargoStylusVersions =
            blockscout_service_launcher::test_server::send_get_request(
                &server.base_url,
                LIST_CARGO_STYLUS_VERSIONS_ROUTE,
            )
            .await;

        assert!(
            response
                .versions
                .iter()
                .any(|version| version.version == "v0.5.0"),
            "v0.5.0 not found in versions: {:?}",
            response.versions
        );
    }
}

mod failure {
    use super::*;
    use stylus_verifier_proto::blockscout::stylus_verifier::v1::verify_response;

    fn assert_failure(response: VerifyResponse, expected_message: &str) {
        match response.verify_response {
            Some(response) => match response {
                verify_response::VerifyResponse::VerificationSuccess(success) => {
                    panic!("invalid response (failure expected): {success:?}")
                }
                verify_response::VerifyResponse::VerificationFailure(failure) => {
                    let message = failure.message;
                    assert!(
                        message.contains(expected_message),
                        "expected_message={expected_message}, found={message}"
                    );
                }
            },
            None => panic!("invalid response: {response:?}"),
        }
    }

    #[tokio::test]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn no_match() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_single_call_with_prefix_0.6.2",
            );

        let server = crate::start_server().await;

        // The deployment of a different contract.
        test_case.deployment_transaction = blockscout_display_bytes::decode_hex(
            "0xa82f7281a601dc1227f9c6ac7a934189e28b5678f3d48749ae7726f7f9effc11",
        )
        .unwrap()
        .into();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "does not match the deployment transaction");
    }

    #[tokio::test]
    async fn malformed_commit_hash() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
            );

        let server = crate::start_server().await;

        // commit hash is not valid hex
        test_case.commit = "qwerty0000001".to_string();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "commit hash not found");
    }

    #[tokio::test]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn unknown_commit_hash() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_single_call_with_prefix_0.6.2",
            );

        let server = crate::start_server().await;

        // commit hash does not exist for the given repository
        test_case.commit = "0000001".to_string();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "commit hash not found");
    }

    #[tokio::test]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn unknown_github_repository() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_single_call_with_prefix_0.6.2",
            );

        let server = crate::start_server().await;

        // repository does not exist
        test_case.repository_url =
            Url::parse("https://github.com/OffchainLabs/unexistent-stylus-hello-world").unwrap();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "repository not found");
    }

    #[tokio::test]
    async fn not_a_github_repository() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
            );

        let server = crate::start_server().await;

        // url is not a github url
        test_case.repository_url =
            Url::parse("https://gitlab.com/OffchainLabs/stylus-hello-world").unwrap();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "url is not a github repository");
    }

    // `OffchainLabs/stylus-hello-world` had no rust-toolchain.toml at this commit.
    #[tokio::test]
    #[ignore = "requires STYLUS_VERIFIER_TEST_SANDBOX"]
    async fn no_rust_toolchain_specified() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_single_call_with_prefix_0.6.2",
            );

        let server = crate::start_server().await;

        test_case.repository_url =
            Url::parse("https://github.com/OffchainLabs/stylus-hello-world").unwrap();
        test_case.commit = "c8b9294f3b59051ff90fc941f8737c926f24ce7c".to_string();
        test_case.path_prefix = String::new();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "rust-toolchain.toml");
    }
}

mod bad_request {
    use super::*;
    use reqwest::StatusCode;

    #[tokio::test]
    async fn invalid_cargo_stylus_version() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
            );
        let server = crate::start_server().await;

        test_case.cargo_stylus_version = "invalid_version".to_string();
        let _response = crate::expect_post_request(
            StatusCode::BAD_REQUEST,
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;

        // v0.0.1 is not supported cargo-stylus version
        test_case.cargo_stylus_version = "v0.0.1".to_string();
        let _response = crate::expect_post_request(
            StatusCode::BAD_REQUEST,
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;

        test_case.cargo_stylus_version = "0.0.1".to_string();
        let _response = crate::expect_post_request(
            StatusCode::BAD_REQUEST,
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
    }
}
