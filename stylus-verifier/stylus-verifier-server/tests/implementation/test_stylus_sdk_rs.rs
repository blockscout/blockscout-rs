use crate::types_stylus_sdk_rs::VerifyGithubRepositoryTestCase;
use stylus_verifier_proto::blockscout::stylus_verifier::v1::VerifyResponse;

use url::Url;

const VERIFY_GITHUB_REPOSITORY_ROUTE: &str = "/api/v1/stylus-sdk-rs:verify-github-repository";

mod verify_github_repository {
    use super::*;

    #[tokio::test]
    async fn stylus_hello_world_0_5_0() {
        let test_case: VerifyGithubRepositoryTestCase = VerifyGithubRepositoryTestCase::from_file(
            "verify_github_repository_stylus_hello_world_0.5.0",
        );

        let server = crate::start_server().await;

        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;

        test_case.check_verify_response(response);
    }

    #[tokio::test]
    async fn with_prefix_0_5_0() {
        let test_case: VerifyGithubRepositoryTestCase = VerifyGithubRepositoryTestCase::from_file(
            "verify_github_repository_single_call_with_prefix_0.5.0",
        );

        let server = crate::start_server().await;

        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;

        test_case.check_verify_response(response);
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
    async fn no_match() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
            );

        let server = crate::start_server().await;

        // The hash corresponds to contract where a new line was added inside the line 1 of `src/lib.rs`
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
        assert_failure(response, "Prelude mismatch");
    }

    #[tokio::test]
    async fn invalid_commit_hash() {
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
    async fn invalid_github_repository() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
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

    #[tokio::test]
    async fn no_rust_toolchain_specified() {
        let mut test_case: VerifyGithubRepositoryTestCase =
            VerifyGithubRepositoryTestCase::from_file(
                "verify_github_repository_stylus_hello_world_0.5.0",
            );

        let server = crate::start_server().await;

        // repository does not exist
        test_case.commit = "c8b9294f3b59051ff90fc941f8737c926f24ce7c".to_string();
        let response: VerifyResponse = blockscout_service_launcher::test_server::send_post_request(
            &server.base_url,
            VERIFY_GITHUB_REPOSITORY_ROUTE,
            &test_case.to_request(),
        )
        .await;
        assert_failure(response, "rust-toolchain.toml");

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
